//! Your Den: the launcher's one Den page and its one-click flow (docs/DEN.md "Your Den").
//!
//! A Den runs in one of two places, and the page treats them the same:
//! * **Den Lite** (host/lite.rs): the built-in Den server, no Docker. The first click plays straight into the white
//!   room. Shown only when the program is on this PC.
//! * **the Docker Den** (the Host pack's `den` profile): what friends can join through the Leyline network. "Share with
//!   friends" moves the Lite Den there ([`move_to_kit`]): every Lite Den file is copied into the Host pack's saves and
//!   the newest is opened there, unchanged.
//!
//! Saved Dens ([`saves_root`]) are the files of whichever Den is in use: the Docker Den's once the Host pack hosts one,
//! else Den Lite's. Packs are the recipes a pack builder leaves in `state/packs` (either root). Add from a link fetches a
//! Den someone shared on the Workshop: a `.den.json` GitHub release file, at most 512 KB, checked like any Den file.

use crate::host::dens::{self, DenFile};
use crate::host::lite::LiteStatus;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Pack {
    pub id: String,
    pub title: String,
    pub line: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DenState {
    pub lite: LiteStatus,
    /// A Host pack is chosen.
    pub kit: bool,
    /// The Host pack hosts the Den (profile `den`).
    pub docker_den: bool,
    /// The Host pack hosts another realm (its name): the Den would take turns with it.
    pub other_realm: Option<String>,
    /// The Docker Den's account (the realm's GM name; the player made it on the join page), or "".
    pub account: String,
    /// "kit" or "lite": where Saved Dens are.
    pub saves: &'static str,
    pub dens: Vec<DenFile>,
    pub packs: Vec<Pack>,
}

fn host_json(kit: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(kit.join("state").join("host.json")).ok()?).ok()
}

/// The Docker Den's account name: the realm's GM (`gm_name`, 3 to 16 letters or digits), else "".
pub fn account(kit: &Path) -> String {
    let g = host_json(kit).and_then(|v| v.get("gm_name").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
    if (3..=16).contains(&g.len()) && g.chars().all(|c| c.is_ascii_alphanumeric()) {
        g.to_ascii_uppercase()
    } else {
        String::new()
    }
}

/// Whose files Saved Dens shows: the Host pack's once it hosts the Den, else Den Lite's.
pub fn saves_root(kit: Option<&Path>, docker_den: bool, lite_root: &Path) -> (PathBuf, &'static str) {
    match kit {
        Some(k) if docker_den => (k.to_path_buf(), "kit"),
        _ => (lite_root.to_path_buf(), "lite"),
    }
}

const PACKS_MAX: usize = 64;
const PACK_BYTES: u64 = 64 * 1024;

fn line(v: &Value, keys: &[&str], n: usize) -> String {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str)).unwrap_or("").chars().filter(|c| !c.is_control()).take(n).collect()
}

/// The pack recipes in `<root>/state/packs/*.json` of each root (a name, a title, a line), by name, each once.
pub fn packs(roots: &[&Path]) -> Vec<Pack> {
    let mut out: Vec<Pack> = Vec::new();
    for r in roots {
        let Ok(rd) = fs::read_dir(r.join("state").join("packs")) else { continue };
        let mut files: Vec<_> = rd.flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for e in files.into_iter().take(PACKS_MAX) {
            let file = e.file_name().to_string_lossy().to_string();
            let Some(stem) = file.strip_suffix(".json") else { continue };
            let Some(id) = dens::clean_name(stem) else { continue };
            if out.iter().any(|p| p.id == id) || e.metadata().map_or(true, |m| !m.is_file() || m.len() > PACK_BYTES) {
                continue;
            }
            let Some(v) = fs::read_to_string(e.path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object) else {
                continue;
            };
            let title = Some(line(&v, &["title", "name"], 48)).filter(|t| !t.trim().is_empty()).unwrap_or_else(|| id.clone());
            out.push(Pack { id, title, line: line(&v, &["line", "summary", "description"], 140) });
        }
    }
    out
}

/// Share with friends: every Den Lite file into the Host pack's saves (a file already there with the same text is
/// skipped, another of the same name is kept and this one added beside it). Returns the name the newest Lite Den has
/// in the Host pack now, for "Open" there, or None when Den Lite had none.
pub fn move_to_kit(lite_root: &Path, kit: &Path) -> Result<Option<String>, String> {
    // the Den's ground (host/terrain.rs) goes along, unchanged, unless the Docker Den has its own already
    let (from, to) = (crate::host::terrain::ground_path(lite_root), crate::host::terrain::ground_path(kit));
    if from.is_file() && !to.exists() {
        fs::create_dir_all(dens::dir(kit)).map_err(|e| format!("Couldn't make the Den folder: {e}"))?;
        fs::copy(&from, &to).map_err(|e| format!("Couldn't move your Den's ground: {e}"))?;
    }
    let mut newest = None;
    for f in dens::list(lite_root) {
        let Ok(text) = dens::text(lite_root, &f.name) else { continue };   // a broken file stays where it is
        let there = dens::text(kit, &f.name).ok();
        let name = if there.as_deref() == Some(text.as_str()) { f.name.clone() } else { dens::add(kit, &format!("{}.den.json", f.name), &text)? };
        if newest.is_none() {
            newest = Some(name);   // dens::list is newest first
        }
    }
    Ok(newest)
}

/// A Den someone shared: `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>.den.json` (what the
/// Workshop lists, stream/listings.py), nothing else. Returns the file's name.
pub fn release_file(url: &str) -> Option<String> {
    if url.len() > 400 || url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '\\' | '%')) {
        return None;
    }
    let rest = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    let part = |s: &str| !s.is_empty() && s.len() <= 100 && !s.starts_with('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if parts.len() != 6 || parts[2] != "releases" || parts[3] != "download" || ![parts[0], parts[1], parts[4], parts[5]].iter().all(|p| part(p)) {
        return None;
    }
    let file = parts[5];
    file.to_ascii_lowercase().ends_with(".den.json").then(|| file.to_string())
}

/// Add from a link: download the shared Den (at most 512 KB, from GitHub and its download hosts only), check it like
/// any Den file, and add it to Saved Dens. `sha256`: the Workshop's fingerprint, when the link came from there.
pub fn fetch_add(root: &Path, url: &str, sha256: Option<&str>, scratch: &Path) -> Result<String, String> {
    let file = release_file(url).ok_or("Paste the link to a Den file on a GitHub release (it ends in .den.json).")?;
    fs::create_dir_all(scratch).map_err(|e| format!("Couldn't make a scratch folder: {e}"))?;
    let tmp = scratch.join(format!("den-fetch-{}.json", std::process::id()));
    let got = crate::upstream::download(url, &tmp, dens::MAX_BYTES);
    let text = got.and_then(|(_, sha)| {
        if sha256.is_some_and(|w| !w.eq_ignore_ascii_case(&sha)) {
            return Err("That Den file changed since it was shared: ask its maker to share it again.".to_string());
        }
        fs::read_to_string(&tmp).map_err(|_| "That isn't a Den file (it can't be read).".to_string())
    });
    let _ = fs::remove_file(&tmp);
    let text = text.map_err(|e| if e.starts_with("That") { e } else { format!("Couldn't get that Den file: {e}") })?;
    dens::add(root, &file, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"format":"leyline-den","version":1,"name":"house","saved":"2026-11-01T01:00:00Z","props":[{"entry":19006,"x":0,"y":24,"z":0}]}"#;
    const OLDER: &str = r#"{"format":"leyline-den","version":1,"name":"yard","saved":"2026-10-30T01:00:00Z","props":[]}"#;

    fn kit_with(host: &str) -> tempfile::TempDir {
        let k = tempfile::tempdir().unwrap();
        fs::create_dir_all(k.path().join("state")).unwrap();
        fs::write(k.path().join("state/host.json"), host).unwrap();
        k
    }

    #[test]
    fn the_dens_account_is_the_realms_gm() {
        assert_eq!(account(kit_with(r#"{"realm_id":"abcdefghijk2345a","profile":"den","gm_name":"bea42"}"#).path()), "BEA42");
        for bad in [r#"{"gm_name":"a b"}"#, r#"{"gm_name":"x"}"#, r#"{}"#, "not json"] {
            assert_eq!(account(kit_with(bad).path()), "", "{bad}");
        }
    }

    #[test]
    fn saved_dens_follow_the_den_in_use() {
        let k = Path::new("/kit");
        let l = Path::new("/lite");
        assert_eq!(saves_root(Some(k), true, l), (k.to_path_buf(), "kit"));
        assert_eq!(saves_root(Some(k), false, l), (l.to_path_buf(), "lite"), "a Host pack hosting another realm: Lite's");
        assert_eq!(saves_root(None, false, l), (l.to_path_buf(), "lite"));
    }

    #[test]
    fn packs_are_read_not_trusted() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let pa = a.path().join("state/packs");
        fs::create_dir_all(&pa).unwrap();
        fs::write(pa.join("goldshire.json"), r#"{"title":"Goldshire","line":"The inn and the square, ready to build around."}"#).unwrap();
        fs::write(pa.join("Bad Name.json"), r#"{"title":"x"}"#).unwrap();
        fs::write(pa.join("broken.json"), "nope").unwrap();
        fs::write(pa.join("list.json"), "[1,2]").unwrap();
        fs::write(pa.join("notes.txt"), "hi").unwrap();
        fs::write(pa.join("big.json"), format!(r#"{{"title":"{}"}}"#, "x".repeat(70 * 1024))).unwrap();
        fs::write(pa.join("plain.json"), format!(r#"{{"name":"Plain\u0007 {}"}}"#, "y".repeat(80))).unwrap();
        let pb = b.path().join("state/packs");
        fs::create_dir_all(&pb).unwrap();
        fs::write(pb.join("goldshire.json"), r#"{"title":"Another Goldshire"}"#).unwrap();
        fs::write(pb.join("docks.json"), r#"{"summary":"Piers and boats."}"#).unwrap();
        let p = packs(&[a.path(), b.path(), Path::new("/nowhere")]);
        let ids: Vec<&str> = p.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, ["goldshire", "plain", "docks"]);
        assert_eq!((p[0].title.as_str(), p[0].line.as_str()), ("Goldshire", "The inn and the square, ready to build around."));
        assert!(p[1].title.starts_with("Plain ") && p[1].title.chars().count() == 48 && !p[1].title.contains('\u{7}'));
        assert_eq!((p[2].title.as_str(), p[2].line.as_str()), ("docks", "Piers and boats."), "no title: its name");
        assert!(packs(&[]).is_empty());
    }

    #[test]
    fn share_with_friends_moves_every_den_unchanged() {
        let lite = tempfile::tempdir().unwrap();
        let kit = tempfile::tempdir().unwrap();
        assert_eq!(move_to_kit(lite.path(), kit.path()).unwrap(), None, "no Lite Den yet: nothing to open");
        dens::add(lite.path(), "house.den.json", GOOD).unwrap();
        dens::add(lite.path(), "yard.den.json", OLDER).unwrap();
        fs::write(dens::dir(lite.path()).join("broken.den.json"), "nope").unwrap();
        assert_eq!(move_to_kit(lite.path(), kit.path()).unwrap().as_deref(), Some("house"), "the newest opens there");
        assert_eq!(dens::text(kit.path(), "house").unwrap(), GOOD, "unchanged");
        assert_eq!(dens::text(kit.path(), "yard").unwrap(), OLDER);
        assert!(!dens::dir(kit.path()).join("broken.den.json").exists());
        assert_eq!(move_to_kit(lite.path(), kit.path()).unwrap().as_deref(), Some("house"), "again: no copies of the same file");
        assert_eq!(dens::list(kit.path()).len(), 2);
        dens::add(kit.path(), "other.den.json", GOOD.replace("19006", "19007").replace("\"house\"", "\"other\"").as_str()).unwrap();
        fs::write(dens::dir(kit.path()).join("house.den.json"), GOOD.replace("19006", "1")).unwrap();
        assert_eq!(move_to_kit(lite.path(), kit.path()).unwrap().as_deref(), Some("house-2"), "a different house there is kept");
        assert_eq!(dens::text(kit.path(), "house-2").unwrap(), GOOD);
        // the ground goes along once, and a Docker Den's own is kept
        let g = crate::host::terrain::ground_path(lite.path());
        fs::write(&g, "{\"ground\":1}").unwrap();
        move_to_kit(lite.path(), kit.path()).unwrap();
        let k = crate::host::terrain::ground_path(kit.path());
        assert_eq!(fs::read_to_string(&k).unwrap(), "{\"ground\":1}");
        fs::write(&g, "{\"ground\":2}").unwrap();
        move_to_kit(lite.path(), kit.path()).unwrap();
        assert_eq!(fs::read_to_string(&k).unwrap(), "{\"ground\":1}");
    }

    #[test]
    fn only_github_release_den_files() {
        assert_eq!(release_file("https://github.com/bea/den/releases/download/v1/garden.den.json").as_deref(), Some("garden.den.json"));
        for bad in [
            "http://github.com/bea/den/releases/download/v1/garden.den.json",
            "https://github.com/bea/den/releases/download/v1/garden.zip",
            "https://github.com/bea/den/raw/main/garden.den.json",
            "https://example.com/bea/den/releases/download/v1/garden.den.json",
            "https://github.com/bea/den/releases/download/v1/../garden.den.json",
            "https://github.com/bea/den/releases/download/v1/garden.den.json?x=1",
            "https://github.com/bea/den/releases/download/v1/a/garden.den.json",
            "https://github.com/bea/den/releases/download/v1/.den.json",
            "",
        ] {
            assert_eq!(release_file(bad), None, "{bad}");
        }
        let root = tempfile::tempdir().unwrap();
        let err = fetch_add(root.path(), "https://example.com/x.den.json", None, &root.path().join("tmp")).unwrap_err();
        assert!(err.contains("GitHub release"), "{err}");
    }
}
