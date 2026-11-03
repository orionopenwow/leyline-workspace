//! The den-test's own way to type in the Den (owner, 1 Nov 19:41: "the den needs a complete and robust smoke test"):
//! tools/den-check sends the chat commands a player would type (`.den place chair`, `.build undo`, `.den save ...`)
//! through two files in the Dens folder instead of the game's keyboard, so the test never depends on which window
//! has the focus. Off unless Den Lite was started for a test (`--test-control`, or `DEN_LITE_TEST_CONTROL=1`, which
//! the launcher's `den-play --test` sets), and only when it listens on this PC alone (lib.rs `start`).
//!
//! * `<dens>/den-lite.test-cmd`: `{"id": <n>, "cmd": ".den place chair"}`, written by the test. Taken (removed) while
//!   the host is in the world, then run as if the host had typed it (world.rs `chat`): the same answers in the game's
//!   chat, the same changes for everyone in the Den. Only `.den` and `.build` commands, one line, up to 120 letters.
//! * `<dens>/den-lite.test-out`: one JSON line per command run: `{"id", "cmd", "said": [the system lines it said],
//!   "created", "destroyed", "props", "look", "size", "t"}`. Never a password: the commands can't carry one.

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;

pub const CMD_FILE: &str = "den-lite.test-cmd";
pub const OUT_FILE: &str = "den-lite.test-out";
const MAX_CMD: usize = 120;
/// What a command said, at most this many lines (`.den` help is 7).
const MAX_SAID: usize = 24;

/// What one command did, collected while it runs on this thread (world.rs `say` and `apply` add to it).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Capture {
    pub said: Vec<String>,
    pub created: usize,
    pub destroyed: usize,
}

thread_local! {
    static CAP: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

/// Runs `f` with this thread's lines and changes collected.
pub fn capture<T>(f: impl FnOnce() -> T) -> (T, Capture) {
    CAP.with(|c| *c.borrow_mut() = Some(Capture::default()));
    let out = f();
    let cap = CAP.with(|c| c.borrow_mut().take()).unwrap_or_default();
    (out, cap)
}

/// A system line said while collecting (else nothing).
pub fn note_said(text: &str) {
    CAP.with(|c| {
        if let Some(cap) = c.borrow_mut().as_mut() {
            if cap.said.len() < MAX_SAID {
                cap.said.push(text.chars().filter(|c| !c.is_control()).take(240).collect());
            }
        }
    });
}

/// A change to the Den while collecting.
pub fn note_change(created: usize, destroyed: usize) {
    CAP.with(|c| {
        if let Some(cap) = c.borrow_mut().as_mut() {
            cap.created += created;
            cap.destroyed += destroyed;
        }
    });
}

/// A command the test may send: one line of `.den ...` or `.build ...`, plain printable letters.
pub fn allowed(cmd: &str) -> bool {
    let c = cmd.trim();
    c.len() <= MAX_CMD
        && c.chars().all(|ch| ch == ' ' || ch.is_ascii_graphic())
        && (c == ".den" || c.starts_with(".den ") || c.starts_with(".build "))
}

/// The test's next command, taken (the file is removed). None when there is none or it isn't one it may send (a
/// refused one is removed too, and answered with an empty line so the test doesn't wait).
pub fn take(dir: &Path) -> Option<(u64, String)> {
    let p = dir.join(CMD_FILE);
    let t = std::fs::read_to_string(&p).ok()?;
    let v: serde_json::Value = match serde_json::from_str(&t) {
        Ok(v) => v,
        // half written: the test writes it in one go (a rename), so this is someone else's: dropped
        Err(_) => {
            let _ = std::fs::remove_file(&p);
            return None;
        }
    };
    let _ = std::fs::remove_file(&p);
    let id = v.get("id")?.as_u64()?;
    let cmd = v.get("cmd")?.as_str()?.trim().to_string();
    if !allowed(&cmd) {
        write_out(dir, &serde_json::json!({ "id": id, "cmd": "", "refused": true, "said": [] }));
        return None;
    }
    Some((id, cmd))
}

pub fn write_out(dir: &Path, line: &serde_json::Value) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(OUT_FILE)) {
        let _ = writeln!(f, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_den_commands_and_what_they_said() {
        for ok in [".den place chair", ".build undo", ".den", ".den save smoketest", ".den load smoketest replace"] {
            assert!(allowed(ok), "{ok}");
        }
        for bad in ["hello", ".server info", ".den\nplace", &format!(".den find {}", "x".repeat(130)), ".denx", ".account onlinelist", ".den é"] {
            assert!(!allowed(bad), "{bad}");
        }
        let ((), cap) = capture(|| {
            note_said("one");
            note_change(2, 1);
            note_said("two\u{7}");
        });
        assert_eq!(cap, Capture { said: vec!["one".into(), "two".into()], created: 2, destroyed: 1 });
        note_said("not collected");
        let ((), cap) = capture(|| ());
        assert!(cap.said.is_empty());

        let d = std::env::temp_dir().join(format!("den-testctl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(take(&d).is_none());
        std::fs::write(d.join(CMD_FILE), r#"{"id":7,"cmd":" .den place chair "}"#).unwrap();
        assert_eq!(take(&d), Some((7, ".den place chair".into())));
        assert!(!d.join(CMD_FILE).exists(), "taken once");
        std::fs::write(d.join(CMD_FILE), r#"{"id":8,"cmd":"hi everyone"}"#).unwrap();
        assert!(take(&d).is_none());
        let out = std::fs::read_to_string(d.join(OUT_FILE)).unwrap();
        assert!(out.contains("\"id\":8") && out.contains("\"refused\":true"), "{out}");
        std::fs::write(d.join(CMD_FILE), "{\"id\":9,").unwrap();
        assert!(take(&d).is_none() && !d.join(CMD_FILE).exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
