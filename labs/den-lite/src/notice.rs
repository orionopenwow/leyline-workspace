//! The Den without chat spam (owner, 1 Nov 23:46: "Jaina was spamming the chat ... make it make sense"): what Den
//! Lite says that nobody typed for (the welcome, the launcher's answers, a friend in your Den, the encouragement
//! moments, Jaina's greeting) goes to the Leyline UI's Den bar (WrathUnleashedUI DenBar.lua) as a small toast, an
//! addon message `wuden toast <kind>|<text>` (ovmod.rs), instead of system lines in the chat.
//!
//! The bar says hello when it opens (`wuden hello <version>`). Until it has, what would be said waits; when no hello
//! came within `Config::ui_wait_ms` of entering the world (a game without the addon, or an older one), it is said in
//! the chat after all, the long way (the quick start's lines). Commands the player typed (`.den ...`) still answer in
//! the chat: they typed them. What the bar or the Builder asked for (an addon message) answers in the bar.
//!
//! The rest of this file is plain bookkeeping, so it is tested here; world.rs sends.

use std::cell::RefCell;
use std::collections::HashSet;

/// What the bar shows a toast as (DenBar.lua KINDS): a plain line, a done thing, a problem, Jaina, a cheer.
pub const KINDS: &[&str] = &["info", "ok", "warn", "jaina", "cheer"];
/// A toast's text, at most (one addon part: ovmod.rs PART is 200 with the kind in front).
pub const TOAST_MAX: usize = 180;
/// The Den bar's own words for its version (DenBar.lua VERSION); any hello counts.
pub const HELLO: (&str, &str) = ("wuden", "hello");

/// The old way, for a game without the bar: what goes in the chat (or the middle of the screen) instead.
#[derive(Debug, Clone, PartialEq)]
pub enum Old {
    /// A system line in the chat.
    Line(String),
    /// Her line ("Jaina says: ...").
    Jaina(String),
    /// The middle of the screen (SMSG_NOTIFICATION): the encouragement moments.
    Middle(String),
}

/// One thing to say: the toast (None: nothing in the bar) and the old way's lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: &'static str,
    pub toast: Option<String>,
    pub old: Vec<Old>,
}

impl Item {
    /// The same words both ways: a toast, or a system line.
    pub fn line(kind: &'static str, text: impl Into<String>) -> Item {
        let t = text.into();
        Item { kind, toast: Some(t.clone()), old: vec![Old::Line(t)] }
    }
    /// Her words: a toast, or her line in the chat.
    pub fn jaina(text: impl Into<String>) -> Item {
        let t = text.into();
        Item { kind: "jaina", toast: Some(t.clone()), old: vec![Old::Jaina(t)] }
    }
    /// An encouragement moment: a toast, or the middle of the screen.
    pub fn cheer(text: impl Into<String>) -> Item {
        let t = text.into();
        Item { kind: "cheer", toast: Some(t.clone()), old: vec![Old::Middle(t)] }
    }
}

/// The toast's data: `<kind>|<text>`, the text plain (no colour codes, no control letters, no `|`), cut short.
pub fn toast_data(kind: &str, text: &str) -> String {
    let kind = if KINDS.contains(&kind) { kind } else { "info" };
    let mut t: String = text.chars().filter(|c| !c.is_control() && *c != '|').collect();
    if t.len() > TOAST_MAX {
        let mut cut = TOAST_MAX - 3;
        while !t.is_char_boundary(cut) {
            cut -= 1;
        }
        t.truncate(cut);
        t.push_str("...");
    }
    format!("{kind}|{t}")
}

struct Waiting {
    sess: u64,
    due_ms: u64,
    items: Vec<Item>,
}

/// Which sessions have the bar, and what waits for a session's hello.
#[derive(Default)]
pub struct Notices {
    ui: HashSet<u64>,
    waiting: Vec<Waiting>,
}

/// Where a thing to say goes now.
#[derive(Debug, PartialEq)]
pub enum Route {
    /// The bar: send the toast.
    Toast,
    /// Kept until the hello (or the wait ends).
    Held,
    /// No bar: the old way.
    Old,
}

impl Notices {
    /// A session entered the world: what it is told waits until `due_ms` for the bar's hello.
    pub fn entered(&mut self, sess: u64, due_ms: u64) {
        self.ui.remove(&sess);
        self.waiting.retain(|w| w.sess != sess);
        self.waiting.push(Waiting { sess, due_ms, items: Vec::new() });
    }

    pub fn has_ui(&self, sess: u64) -> bool {
        self.ui.contains(&sess)
    }

    /// Where `item` goes for `sess` (held ones are kept here).
    pub fn route(&mut self, sess: u64, item: &Item) -> Route {
        if self.ui.contains(&sess) {
            return Route::Toast;
        }
        if let Some(w) = self.waiting.iter_mut().find(|w| w.sess == sess) {
            w.items.push(item.clone());
            return Route::Held;
        }
        Route::Old
    }

    /// The bar said hello: from now on toasts; what waited, to show now (in order).
    pub fn hello(&mut self, sess: u64) -> Vec<Item> {
        self.ui.insert(sess);
        let mut out = Vec::new();
        self.waiting.retain_mut(|w| {
            if w.sess == sess {
                out.append(&mut w.items);
                false
            } else {
                true
            }
        });
        out
    }

    /// The sessions whose wait ended without a hello, and what they are told the old way.
    pub fn due(&mut self, now_ms: u64) -> Vec<(u64, Vec<Item>)> {
        let mut out = Vec::new();
        self.waiting.retain_mut(|w| {
            if now_ms >= w.due_ms {
                out.push((w.sess, std::mem::take(&mut w.items)));
                false
            } else {
                true
            }
        });
        out
    }

    /// The session ended.
    pub fn gone(&mut self, sess: u64) {
        self.ui.remove(&sess);
        self.waiting.retain(|w| w.sess != sess);
    }
}

/// The encouragement moments, rarely: at most one every `gap` seconds, never two at once. True: one may be said now
/// (and `last` is moved on).
pub fn cheer_ok(last: &mut u64, now: u64, gap: u64) -> bool {
    if *last != 0 && now.saturating_sub(*last) < gap {
        return false;
    }
    *last = now;
    true
}

// ---------------------------------------------------------------- answers to the bar, out of the chat
//
// While Den Lite answers an addon message from a session with the bar (world.rs `addon`), the system lines it would
// say to that session are kept here instead (world.rs `say` asks `kept`), then sent as toasts.

thread_local! {
    static KEEP: RefCell<Option<(usize, Vec<String>)>> = const { RefCell::new(None) };
}

/// Runs `f` with the lines said to the writer `who` (its address) kept, not said; returns them.
pub fn keeping<T>(who: usize, f: impl FnOnce() -> T) -> (T, Vec<String>) {
    KEEP.with(|k| *k.borrow_mut() = Some((who, Vec::new())));
    let out = f();
    let lines = KEEP.with(|k| k.borrow_mut().take()).map(|(_, l)| l).unwrap_or_default();
    (out, lines)
}

/// A system line about to go to `who`: true when it is kept (so it isn't said).
pub fn kept(who: usize, text: &str) -> bool {
    KEEP.with(|k| match k.borrow_mut().as_mut() {
        Some((w, lines)) if *w == who => {
            if lines.len() < 8 {
                lines.push(text.to_string());
            }
            true
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_until_the_hello_then_toasts() {
        let mut n = Notices::default();
        n.entered(1, 4000);
        let hi = Item::line("info", "Welcome.");
        assert_eq!(n.route(1, &hi), Route::Held);
        assert_eq!(n.route(1, &Item::jaina("I'm with you.")), Route::Held);
        assert_eq!(n.route(2, &hi), Route::Old, "another session never entered: the old way");
        let shown = n.hello(1);
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[1].kind, "jaina");
        assert!(n.has_ui(1));
        assert_eq!(n.route(1, &hi), Route::Toast);
        assert!(n.due(10_000).is_empty(), "nothing waits any more");
    }

    #[test]
    fn no_hello_in_time_the_old_way() {
        let mut n = Notices::default();
        n.entered(3, 4000);
        n.route(3, &Item::line("info", "Welcome."));
        assert!(n.due(3999).is_empty());
        let d = n.due(4000);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].1[0].old, vec![Old::Line("Welcome.".into())]);
        assert_eq!(n.route(3, &Item::line("info", "x")), Route::Old);
        // a hello later still turns the bar on
        assert!(n.hello(3).is_empty());
        assert_eq!(n.route(3, &Item::line("info", "x")), Route::Toast);
        n.gone(3);
        assert_eq!(n.route(3, &Item::line("info", "x")), Route::Old);
        // entering again waits again
        n.entered(3, 100);
        assert_eq!(n.route(3, &Item::cheer("Nice.")), Route::Held);
    }

    #[test]
    fn a_toast_is_plain_and_short() {
        assert_eq!(toast_data("ok", "Saved."), "ok|Saved.");
        assert_eq!(toast_data("nope", "a|b\u{7}c"), "info|abc");
        let long = toast_data("info", &"é".repeat(200));
        assert!(long.len() <= 5 + TOAST_MAX && long.ends_with("..."), "{}", long.len());
        assert!(crate::ovmod::answer_bodies(1, "wuden", "toast", &long).len() == 1, "one addon part");
    }

    #[test]
    fn cheers_rarely_never_stacked() {
        let mut last = 0;
        assert!(cheer_ok(&mut last, 1000, 180));
        assert!(!cheer_ok(&mut last, 1010, 180), "not right after another");
        assert!(!cheer_ok(&mut last, 1179, 180));
        assert!(cheer_ok(&mut last, 1180, 180));
        let mut l = 0;
        assert!(cheer_ok(&mut l, 5, 0) && cheer_ok(&mut l, 5, 0), "no gap: each one");
    }

    #[test]
    fn answers_to_the_bar_are_kept_not_said() {
        let ((), lines) = keeping(7, || {
            assert!(kept(7, "Placed."));
            assert!(!kept(8, "to someone else"), "only the asker's lines");
        });
        assert_eq!(lines, vec!["Placed.".to_string()]);
        assert!(!kept(7, "after: said"));
    }
}
