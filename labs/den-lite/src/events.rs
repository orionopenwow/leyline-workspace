//! `<dens>/den-lite.events`: what happened on the way into the Den, one JSON line per milestone, for the launcher's
//! one-click start (it types the local password or presses Enter only when a step doesn't come by itself) and for
//! the owner's `den-test` (tools/den-check). A new file at every start; at most [`MAX_LINES`] lines.
//!
//! Lines: `{"t": <unix ms>, "ev": "<name>", "who": "<account or character>", "detail": "..."}`. The names:
//!
//! | ev | when |
//! |---|---|
//! | `start` | Den Lite listens (detail: the login port) |
//! | `login_challenge` | a game asked to sign in (who: the account) |
//! | `auth_ok` / `auth_fail` | the password was right / wrong |
//! | `realm_list` | the realm list was sent |
//! | `world_auth` | the game came to the world server with its session |
//! | `char_enum` | the character list was sent (detail: how many) |
//! | `char_create` | the game made a character (Den Lite keeps its ready-made one) |
//! | `player_login` | the game asked to enter the world with the character |
//! | `in_world` | the world was sent: the player stands in the Den |
//! | `first_move` | the first movement from the game in this session |
//! | `builder_ready` | the in-game Den panel or Builder asked Den Lite (addon channel) and was answered |
//! | `addon` | an addon message from the game (detail: its mod and name; never its data) |
//! | `ask` | the game asked the launcher for something (detail: `invite` or `forge`) |
//! | `peer_seen` | one player was shown to another (who: the one seeing; detail: the one seen) |
//! | `answer` | the launcher's answer went into the game (detail: `invite` or `say`; never the link) |
//! | `den_change` | the Den changed: a piece placed or taken back, a load, the room rebuilt (detail: `+<made> -<gone>`) |
//! | `den_save` / `den_load` | `.den save <name>` / `.den load <name>` (detail: the name, and `replace` or `add`) |
//! | `autosave` | the Den was written to `autosave.den.json` (detail: how many props) |
//! | `test_cmd` | the den-test sent a command (testctl.rs; detail: its first two words) |
//! | `logout` | the game logged out |
//!
//! Nothing secret is ever written: no password, no session key, no addon data.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE: &str = "den-lite.events";
pub const MAX_LINES: usize = 4000;

pub struct Events {
    path: Option<PathBuf>,
    out: Mutex<(Option<std::fs::File>, usize)>,
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>()
}

impl Events {
    /// A new log in `dir` (the old one is replaced).
    pub fn new(dir: &Path) -> Events {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join(FILE);
        let f = std::fs::File::create(&path).ok();
        Events { path: Some(path), out: Mutex::new((f, 0)) }
    }

    /// No log (nothing is written).
    pub fn none() -> Events {
        Events { path: None, out: Mutex::new((None, 0)) }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn note(&self, ev: &str, who: &str, detail: &str) {
        let mut g = self.out.lock().unwrap_or_else(|e| e.into_inner());
        if g.1 >= MAX_LINES {
            return;
        }
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let line = serde_json::json!({ "t": t, "ev": clean(ev, 24), "who": clean(who, 32), "detail": clean(detail, 120) });
        if let Some(f) = g.0.as_mut() {
            if writeln!(f, "{line}").and_then(|_| f.flush()).is_ok() {
                g.1 += 1;
            }
        }
    }
}

/// One line of the log, read back (tests and the launcher's own copy of this reader).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Line {
    pub t: u64,
    pub ev: String,
    #[serde(default)]
    pub who: String,
    #[serde(default)]
    pub detail: String,
}

pub fn read(dir: &Path) -> Vec<Line> {
    std::fs::read_to_string(dir.join(FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_per_milestone_and_nothing_else() {
        let d = std::env::temp_dir().join(format!("den-ev-{}", std::process::id()));
        let e = Events::new(&d);
        e.note("auth_ok", "DEN", "");
        e.note("addon", "Wren\n", "wuden info");
        let l = read(&d);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].ev.as_str(), l[0].who.as_str()), ("auth_ok", "DEN"));
        assert_eq!(l[1].who, "Wren", "no control characters");
        let e2 = Events::new(&d);
        e2.note("start", "", "3724");
        assert_eq!(read(&d).len(), 1, "a new file at every start");
        Events::none().note("x", "", "");
        let _ = std::fs::remove_dir_all(&d);
    }
}
