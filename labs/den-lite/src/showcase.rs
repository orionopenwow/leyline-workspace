//! The showcase (docs/DEN-MARKETPLACE.md "Pictures"): when a player shares a Den from the launcher, the game takes its
//! pictures by itself. The launcher writes `<dens>/showcase.json` (`{"id": "<letters>", "file": ""}`, denmarket.rs
//! `showcase_request`); the open.json watcher (world.rs `watch_open`) sees it while the host is in the world and:
//!
//! 1. puts the far camera's object (farsight.rs) over the middle of what is built ([`pose`]: the props' box, a little
//!    above it) and sets the view on it, as the Builder's snap out does;
//! 2. tells the Den bar (DenBar.lua, `wuden` / `showcase`): `<shots>|<ms>`. The addon hides the interface, zooms the
//!    camera out to what the Den needs, then [`SHOTS`] times: waits, presses the game's own Screenshot(), and turns the
//!    camera a quarter round (FlipCameraYaw); then shows the interface again;
//! 3. after [`total_ms`] gives the view back and writes `<dens>/showcase-done.json` (`{"id", "shots", "radius"}`).
//!
//! The game writes the pictures into its own Screenshots folder; the launcher takes the newest ones since its ask
//! (denmarket.rs `find_shots`), shows them, and sends only those the player keeps. Nothing leaves the PC by itself.

use serde_json::Value;
use std::fs;
use std::path::Path;

pub const ASK: &str = "showcase.json";
pub const DONE: &str = "showcase-done.json";
/// Four sides of the build.
pub const SHOTS: u32 = 4;
/// Between two pictures: the camera settles, the picture is written.
pub const STEP_MS: u32 = 1800;
/// Before the first: the view moves to the camera object and the interface hides.
pub const LEAD_MS: u32 = 1500;

/// The launcher's ask: its id (letters, digits, - and _, up to 64), once.
pub fn take_ask(dir: &Path) -> Option<String> {
    let p = dir.join(ASK);
    let meta = fs::metadata(&p).ok()?;
    let text = if meta.len() <= 4096 { fs::read_to_string(&p).ok() } else { None };
    let _ = fs::remove_file(&p);
    let v: Value = serde_json::from_str(text?.trim_start_matches('\u{feff}')).ok()?;
    let id = v.get("id")?.as_str()?.to_string();
    (!id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then_some(id)
}

/// Where the camera object goes and how far the camera zooms out: (x, y, z, radius) for the built pieces at
/// `points` (world x, y, z), or the Den's middle `home` when nothing is built. The radius (yards, 8 to 50: the game's
/// own camera limit) is the farthest piece from the middle, plus room.
pub fn pose(points: &[(f32, f32, f32)], home: (f32, f32, f32)) -> (f32, f32, f32, f32) {
    if points.is_empty() {
        return (home.0, home.1, home.2 + 2.0, 12.0);
    }
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX, f32::MAX), (f32::MIN, f32::MIN, f32::MIN));
    for &(x, y, z) in points {
        lo = (lo.0.min(x), lo.1.min(y), lo.2.min(z));
        hi = (hi.0.max(x), hi.1.max(y), hi.2.max(z));
    }
    let mid = ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0);
    let far = points.iter().map(|&(x, y, _)| ((x - mid.0).powi(2) + (y - mid.1).powi(2)).sqrt()).fold(0.0, f32::max);
    // a little above the ground of the build (a third of its height), so the pictures look down on it
    let z = lo.2 + ((hi.2 - lo.2) / 3.0).max(1.5);
    (mid.0, mid.1, z, (far * 1.4 + 6.0).clamp(8.0, 50.0))
}

/// The Den bar's line: "<shots>|<step ms>|<zoom yards>".
pub fn addon_line(radius: f32) -> String {
    format!("{SHOTS}|{STEP_MS}|{}", radius.round() as u32)
}

/// How long the whole showcase takes before the view is given back.
pub fn total_ms() -> u32 {
    LEAD_MS + SHOTS * STEP_MS + 1200
}

/// The answer for the launcher.
pub fn done_doc(id: &str, radius: f32, ok: bool) -> String {
    serde_json::json!({ "id": id, "shots": if ok { SHOTS } else { 0 }, "radius": radius.round(), "ok": ok }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ask_is_taken_once_and_checked() {
        let d = std::env::temp_dir().join(format!("den-lite-showcase-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(ASK), r#"{"id":"sc18f2a","file":""}"#).unwrap();
        assert_eq!(take_ask(&d).as_deref(), Some("sc18f2a"));
        assert_eq!(take_ask(&d), None, "once");
        for bad in [r#"{"id":"../x"}"#, r#"{"id":""}"#, "nope", r#"{"file":"x"}"#] {
            fs::write(d.join(ASK), bad).unwrap();
            assert_eq!(take_ask(&d), None, "{bad}");
            assert!(!d.join(ASK).exists(), "a bad ask is gone too");
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_camera_looks_at_the_middle_of_what_is_built() {
        let (x, y, z, r) = pose(&[(10.0, 0.0, 5.0), (20.0, 10.0, 5.0), (10.0, 10.0, 8.0)], (0.0, 0.0, 0.0));
        assert_eq!((x, y), (15.0, 5.0));
        assert!((z - 6.5).abs() < 0.01, "{z}");
        assert!((r - (50f32.sqrt() * 1.4 + 6.0)).abs() < 0.01, "{r}");
        let (_, _, _, small) = pose(&[(1.0, 1.0, 0.0)], (0.0, 0.0, 0.0));
        assert_eq!(small, 8.0, "at least 8 yards");
        let (_, _, _, huge) = pose(&[(-200.0, 0.0, 0.0), (200.0, 0.0, 0.0)], (0.0, 0.0, 0.0));
        assert_eq!(huge, 50.0, "the game's own camera limit");
        assert_eq!(pose(&[], (3.0, 4.0, 5.0)), (3.0, 4.0, 7.0, 12.0), "nothing built: the Den's middle");
        assert_eq!(addon_line(23.6), format!("4|{STEP_MS}|24"));
        assert!(total_ms() > SHOTS * STEP_MS);
        let v: Value = serde_json::from_str(&done_doc("sc1", 23.6, true)).unwrap();
        assert_eq!((v["id"].as_str(), v["shots"].as_u64()), (Some("sc1"), Some(4)));
    }
}
