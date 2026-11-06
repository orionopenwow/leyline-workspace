//! Bots always up to date (owner 4 Nov, Single player Phase 4; docs/BOTS-UPDATES.md): which bots this PC's realm
//! runs, whether newer ones are out, and getting them (the Single player page's Manage card: ui/singleupdate.js).
//!
//! * The bots' pins: the mod-playerbots commit and its core's (realm/bots.json, `{"format":1, playerbots, core,
//!   date, changes}`). New pins reach players only after the owner merges the weekly canary's pull request
//!   (bots-canary.yml); wu-images.yml then builds the `:stable` images with the pins as labels
//!   (`org.leyline.playerbots`, `org.leyline.core`).
//! * What is out: the Host pack listing's `bots` (pack.rs Descriptor, from make-hostpack.py) when it is newer than
//!   the installed pack, else the installed kit's own `realm-files/bots.json`.
//! * What runs here: the labels of the kit's worldserver image (`docker image inspect`), else what our last pull
//!   wrote (`cache/bots-engine.json`, kept across pack updates). Unknown: nothing is offered (never a nag).
//! * Updating: only when the player presses it. `pull` fetches the three realm images of the kit's own image and tag
//!   (never another name); a realm that is running keeps running the old ones until it is restarted, which the
//!   player chooses (the page offers it; never automatic). Prebuilt images only: a PC that built its own images, or
//!   runs stock ones, is told so instead.
//!
//! Nothing here takes a name, a path or a URL from the UI. The kit's `.env` is read for WU_IMAGE and WU_TAG only.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Our prebuilt images (wu-images.yml). Only these are pulled.
pub const PREBUILT: &str = "ghcr.io/orionopenwow/wu-ac-wotlk";
const ENGINE_FILE: &str = "bots-engine.json";
const IMAGES: [&str; 3] = ["worldserver", "authserver", "db-import"];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pins {
    pub playerbots: String,
    pub core: String,
    /// When they were promoted (YYYY-MM-DD), for "Bots updated"
    pub date: String,
    /// Upstream changes since the pins before (a count only: upstream's words are never shown)
    pub changes: Option<u32>,
}

fn sha_ok(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn date_ok(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

impl Pins {
    /// Pins made safe (full commits; a date only when it looks like one), or None.
    pub fn clean(self) -> Option<Pins> {
        (sha_ok(&self.playerbots) && sha_ok(&self.core)).then(|| Pins {
            date: if date_ok(&self.date) { self.date } else { String::new() },
            changes: self.changes.filter(|n| *n < 100_000),
            ..self
        })
    }
    pub fn same(&self, o: &Pins) -> bool {
        self.playerbots == o.playerbots && self.core == o.core
    }
}

/// realm/bots.json as the kit or the pack listing carries it.
pub fn from_manifest(text: &str) -> Option<Pins> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    if v.get("format").and_then(|f| f.as_u64()) != Some(1) {
        return None;
    }
    serde_json::from_value::<Pins>(v).ok()?.clean()
}

/// `docker image inspect --format '{{json .Config.Labels}}'` output.
pub fn from_labels(text: &str) -> Option<Pins> {
    let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Pins { playerbots: s("org.leyline.playerbots"), core: s("org.leyline.core"), ..Default::default() }.clean()
}

/// WU_IMAGE and WU_TAG from the kit's `.env` (setup.py writes them); nothing else is read from it.
pub fn kit_image(kit: &Path) -> (String, String) {
    let text = fs::read_to_string(kit.join(".env")).unwrap_or_default();
    let get = |k: &str| {
        text.lines()
            .filter_map(|l| l.trim().strip_prefix(k).and_then(|r| r.strip_prefix('=')))
            .map(|v| v.trim().trim_matches('"').to_string())
            .last()
            .unwrap_or_default()
    };
    let (i, t) = (get("WU_IMAGE"), get("WU_TAG"));
    (if i.is_empty() { "ov/ac-wotlk".into() } else { i }, if t.is_empty() { "custom".into() } else { t })
}

fn tag_ok(t: &str) -> bool {
    !t.is_empty() && t.len() <= 64 && t.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

pub fn prebuilt(image: &str, tag: &str) -> bool {
    image == PREBUILT && tag_ok(tag)
}

fn engine_file(kit: &Path) -> PathBuf {
    kit.join("cache").join(ENGINE_FILE)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BotsStatus {
    /// What this PC's realm images carry (None: not known)
    pub engine: Option<Pins>,
    /// The newest promoted bots (None: no listing and no kit manifest)
    pub latest: Option<Pins>,
    /// "Update the bots" is offered
    pub update: bool,
    /// The newer bots come with a newer Host pack (its scripts too): update the pack first
    pub pack_first: bool,
    /// The kit runs our prebuilt images (else nothing can be pulled)
    pub prebuilt: bool,
    /// Said under the card when there is nothing to press
    pub note: Option<String>,
}

/// The decision, from what is known.
pub fn decide(engine: Option<Pins>, kit_pins: Option<Pins>, listed: Option<Pins>, pack_update: bool, prebuilt: bool) -> BotsStatus {
    // a newer pack's pins count only when the pack itself is newer (an older listing never "updates" backwards)
    let pack_first = pack_update && listed.as_ref().is_some_and(|l| kit_pins.as_ref().map_or(true, |k| !k.same(l)));
    let latest = if pack_first { listed } else { kit_pins.or(listed) };
    let newer = match (&engine, &latest) {
        (Some(e), Some(l)) => !e.same(l),
        _ => false,
    };
    let note = if !prebuilt {
        Some("This realm runs images built on this PC (or stock ones): its bots update when you build it again.".into())
    } else if engine.is_none() && latest.is_some() {
        Some("Leyline can't tell which bots this realm has yet: they are checked again after its next update.".into())
    } else {
        None
    };
    BotsStatus { update: prebuilt && (newer || pack_first), pack_first: prebuilt && pack_first, engine, latest, prebuilt, note }
}

/// The status for `kit`. `inspect(image_ref)` answers the image's labels JSON (None: no such image here).
pub fn status(kit: &Path, listed: Option<Pins>, pack_update: bool, inspect: &dyn Fn(&str) -> Option<String>) -> BotsStatus {
    let (image, tag) = kit_image(kit);
    let pre = prebuilt(&image, &tag);
    let kit_pins = fs::read_to_string(kit.join("realm-files").join("bots.json")).ok().and_then(|t| from_manifest(&t));
    let labelled = if pre { inspect(&format!("{image}-worldserver:{tag}")).and_then(|l| from_labels(&l)) } else { None };
    let recorded = fs::read_to_string(engine_file(kit)).ok().and_then(|t| from_manifest(&t));
    let engine = labelled.map(|mut p| {
        // the labels carry no date: the manifest that names the same pins has it
        if let Some(m) = [&kit_pins, &listed, &recorded].into_iter().flatten().find(|m| m.same(&p)) {
            p.date = m.date.clone();
            p.changes = m.changes;
        }
        p
    });
    decide(engine.or(recorded), kit_pins, listed, pack_update, pre)
}

/// Pull the kit's three realm images (its own image and tag; prebuilt only), then write down what they carry.
/// `run(args)` runs docker. Returns the sentence for the page.
pub fn pull(kit: &Path, latest: Option<&Pins>, run: &dyn Fn(&[&str]) -> Result<String, String>) -> Result<String, String> {
    let (image, tag) = kit_image(kit);
    if !prebuilt(&image, &tag) {
        return Err("This realm runs images built on this PC: build it again to update its bots.".into());
    }
    for part in IMAGES {
        let r = format!("{image}-{part}:{tag}");
        run(&["pull", "-q", &r]).map_err(|e| format!("Could not get the new bots ({e}). Is this PC online?"))?;
    }
    let world = format!("{image}-worldserver:{tag}");
    let got = run(&["image", "inspect", "--format", "{{json .Config.Labels}}", &world]).ok().and_then(|l| from_labels(&l));
    let mut rec = got.or_else(|| latest.cloned());
    if let (Some(r), Some(l)) = (rec.as_mut(), latest) {
        if r.same(l) {
            r.date = l.date.clone();
            r.changes = l.changes;
        }
    }
    if let Some(r) = rec {
        let _ = fs::create_dir_all(kit.join("cache"));
        let mut v = serde_json::to_value(&r).unwrap_or_default();
        v["format"] = 1.into();
        let _ = fs::write(engine_file(kit), serde_json::to_vec_pretty(&v).unwrap_or_default());
    }
    Ok("The new bots are downloaded. They join at the next start of Single player.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const A: &str = "e88cc4099e69f35126b28942a412ca702d20453b";
    const B: &str = "05a1f6fceb8a32aa879b13f5f48934c6e46a774c";
    const C: &str = "3f2969ddb0da42b48b4fca050c02b19163ba1e20";

    fn pins(pb: &str, date: &str) -> Pins {
        Pins { playerbots: pb.into(), core: C.into(), date: date.into(), changes: Some(3) }
    }
    fn manifest(pb: &str, date: &str) -> String {
        format!(r#"{{"format":1,"playerbots":"{pb}","core":"{C}","date":"{date}","changes":3}}"#)
    }
    fn kit(env: &str, kit_pb: Option<&str>) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(".env"), env).unwrap();
        if let Some(pb) = kit_pb {
            fs::create_dir_all(d.path().join("realm-files")).unwrap();
            fs::write(d.path().join("realm-files/bots.json"), manifest(pb, "2026-11-04")).unwrap();
        }
        d
    }
    const ENV: &str = "DB_ROOT_PASSWORD=secret\nWU_IMAGE=ghcr.io/orionopenwow/wu-ac-wotlk\nWU_TAG=stable\n";
    fn labels(pb: &str) -> String {
        format!(r#"{{"org.leyline.playerbots":"{pb}","org.leyline.core":"{C}","org.opencontainers.image.source":"x"}}"#)
    }

    #[test]
    fn pins_are_checked() {
        assert_eq!(from_manifest(&manifest(A, "2026-11-04")), Some(pins(A, "2026-11-04")));
        assert!(from_manifest(&manifest("e88cc40", "2026-11-04")).is_none());
        assert!(from_manifest(&manifest(A, "2026-11-04").replace("\"format\":1", "\"format\":2")).is_none());
        assert_eq!(from_manifest(&manifest(A, "<b>soon</b>")).unwrap().date, "");
        assert_eq!(from_labels(&labels(B)).unwrap().playerbots, B);
        assert!(from_labels("null").is_none());
        assert!(from_labels(r#"{"org.opencontainers.image.source":"x"}"#).is_none());
    }

    #[test]
    fn only_image_and_tag_are_read_from_the_env() {
        let k = kit(ENV, None);
        assert_eq!(kit_image(k.path()), (PREBUILT.to_string(), "stable".to_string()));
        let k = kit("WU_IMAGE=\"ov/ac-wotlk\"\n", None);
        assert_eq!(kit_image(k.path()), ("ov/ac-wotlk".to_string(), "custom".to_string()));
        assert!(prebuilt(PREBUILT, "stable") && !prebuilt("acore/ac-wotlk", "master") && !prebuilt(PREBUILT, "x;rm"));
    }

    #[test]
    fn newer_bots_are_offered_older_or_unknown_never() {
        let none = |_: &str| -> Option<String> { None };
        // the image carries older pins than the kit's manifest: offered
        let k = kit(ENV, Some(B));
        let s = status(k.path(), None, false, &|r: &str| (r == "ghcr.io/orionopenwow/wu-ac-wotlk-worldserver:stable").then(|| labels(A)));
        assert!(s.update && !s.pack_first && s.prebuilt, "{s:?}");
        assert_eq!(s.latest.as_ref().unwrap().playerbots, B);
        // the same: nothing
        let s = status(k.path(), None, false, &|_: &str| Some(labels(B)));
        assert!(!s.update && s.note.is_none(), "{s:?}");
        assert_eq!(s.engine.unwrap().date, "2026-11-04");   // the date from the manifest naming the same pins
        // unknown engine: no offer, a note
        let s = status(k.path(), None, false, &none);
        assert!(!s.update && s.note.is_some());
        // a built-here realm: no offer, said so
        let k2 = kit("WU_IMAGE=ov/ac-wotlk\nWU_TAG=custom\n", Some(B));
        let s = status(k2.path(), None, false, &|_: &str| Some(labels(A)));
        assert!(!s.update && !s.prebuilt && s.note.unwrap().contains("built on this PC"));
    }

    #[test]
    fn a_newer_pack_brings_its_bots() {
        let k = kit(ENV, Some(A));
        let listed = Some(pins(B, "2026-11-10"));
        let s = status(k.path(), listed.clone(), true, &|_: &str| Some(labels(A)));
        assert!(s.update && s.pack_first, "{s:?}");
        assert_eq!(s.latest.unwrap().date, "2026-11-10");
        // the listing's pins without a newer pack: the kit's own decide
        let s = status(k.path(), listed, false, &|_: &str| Some(labels(A)));
        assert!(!s.update);
    }

    #[test]
    fn pull_gets_the_kits_own_images_and_records_them() {
        let k = kit(ENV, Some(B));
        let calls = RefCell::new(Vec::<String>::new());
        let run = |a: &[&str]| -> Result<String, String> {
            calls.borrow_mut().push(a.join(" "));
            Ok(if a[0] == "image" { labels(B) } else { String::new() })
        };
        let latest = pins(B, "2026-11-10");
        let msg = pull(k.path(), Some(&latest), &run).unwrap();
        assert!(msg.contains("next start"));
        assert_eq!(
            calls.borrow()[..3],
            [
                "pull -q ghcr.io/orionopenwow/wu-ac-wotlk-worldserver:stable",
                "pull -q ghcr.io/orionopenwow/wu-ac-wotlk-authserver:stable",
                "pull -q ghcr.io/orionopenwow/wu-ac-wotlk-db-import:stable"
            ]
        );
        let rec = from_manifest(&fs::read_to_string(engine_file(k.path())).unwrap()).unwrap();
        assert_eq!(rec, latest);
        // what was recorded is what runs when the labels can't be read
        let s = status(k.path(), None, false, &|_: &str| None);
        assert_eq!(s.engine.unwrap().playerbots, B);
        assert!(!s.update);
        // offline: said plainly, nothing recorded
        let k3 = kit(ENV, Some(B));
        let err = pull(k3.path(), Some(&latest), &|_: &[&str]| Err("dial tcp: no such host".into())).unwrap_err();
        assert!(err.contains("online"));
        assert!(!engine_file(k3.path()).exists());
        // never for images built here
        let k4 = kit("WU_IMAGE=ov/ac-wotlk\n", Some(B));
        assert!(pull(k4.path(), Some(&latest), &|_: &[&str]| Ok(String::new())).is_err());
    }
}
