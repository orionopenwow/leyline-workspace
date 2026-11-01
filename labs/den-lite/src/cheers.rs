//! Encouragement moments: one short line, the first time something good happens in your Den (your first visit, the
//! first piece you place, the first save, the first time the room grows). Each is said once, ever: what was said is
//! kept in `<dens>/.cheers-done` (one key a line), so a restart never says it again. The launcher's own moments
//! (your first server live, a friend joins, your first mod) are in launcher/ui/guide.js.
//!
//! The line shows in the middle of the screen (SMSG_NOTIFICATION), not in the chat frame, so it never pushes the
//! answer to a command out of view and it fades by itself.

use std::io::Write;
use std::path::Path;

/// The moments and their lines: the one list, the owner edits them here.
/// TODO(owner): a voice pass on every line below, his words.
pub const LINES: &[(&str, &str)] = &[
    ("first-visit", "Welcome home. This room is yours: build anything."),
    ("first-piece", "Your first piece is down. Keep going!"),
    ("first-save", "Saved. Your Den is safe in a file now."),
    ("first-grow", "More room to build. Nice."),
];

const FILE: &str = ".cheers-done";

/// The line for `key` the first time it is asked for in this Den folder, then None. Unknown keys: None.
pub fn once(dir: &Path, key: &str) -> Option<&'static str> {
    let line = LINES.iter().find(|(k, _)| *k == key).map(|(_, l)| *l)?;
    let done = dir.join(FILE);
    if std::fs::read_to_string(&done).map(|t| t.lines().any(|l| l == key)).unwrap_or(false) {
        return None;
    }
    let _ = std::fs::create_dir_all(dir);
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&done).ok()?;
    writeln!(f, "{key}").ok()?;
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_once_kept_across_restarts() {
        let d = std::env::temp_dir().join(format!("den-lite-cheers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(once(&d, "first-save"), Some("Saved. Your Den is safe in a file now."));
        assert_eq!(once(&d, "first-save"), None);
        assert!(once(&d, "first-piece").is_some());
        assert_eq!(once(&d, "nope"), None);
        assert_eq!(std::fs::read_to_string(d.join(FILE)).unwrap(), "first-save\nfirst-piece\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn lines_are_short_and_plain() {
        let brands = ["warcraft", "lich king", "blizzard"];
        for (k, l) in LINES {
            assert!(l.len() <= 80, "{k}");
            let low = l.to_lowercase();
            assert!(brands.iter().all(|b| !low.contains(b)), "{k}");
            assert!(!low.split(|c: char| !c.is_alphanumeric()).any(|w| w == "wow"), "{k}");
            assert!(!l.chars().any(|c| c.is_ascii_digit()), "{k}: no numbers");
        }
    }
}
