//! The companion in Den Lite: one creature that stands by the player, follows loosely and says lines, and the same
//! file channel as the full Den's `ov_den_companion.lua`, so the player's own assistant ("Ask Claude", the launcher's
//! `Leyline mcp` Den tools in launcher/src-tauri/src/agents/den.rs) works here too. In game she is Jaina (the owner's
//! choice, 1 Nov 2026: the game's own model and name, in game only; the launcher and the website say "your
//! companion").
//!
//! Files in the Dens folder (JSON, never run), as den.rs reads and writes them:
//!   `companion-in.json`   written here every 10 seconds and after each ask: `format` "leyline-companion",
//!                         `version` 1, `server` "den-lite", `at` (the heartbeat), `companion` (out or not), `owner`
//!                         (online, inside, x, y from the Den's middle, z from its floor, yaw in degrees), `room`
//!                         (props, half, size, look), `said` (the last line taken), `asks` (the last 20:
//!                         {n, at, text, where}).
//!   `companion-out.json`  den.rs writes (`den_say`): {lines: [{n, at, text}], seen, read_at}. Lines after `said`
//!                         and under 5 minutes old are said, one every 3 seconds, while she is out; `read_at` within
//!                         10 minutes means someone is listening.
//! Den Lite doesn't take `companion-place.json` (a build piece by piece): den_place_layout needs the full Den's
//! catalog and budgets (README).
//!
//! The creature (AzerothCore world database, as ov_den_companion.lua: creature_template 32364, its model 30865):
//! entry and display only; the name the client shows is the one this server answers its query with.

use serde_json::{json, Value};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::fs;
use std::path::Path;

pub const ENTRY: u32 = 32364;
pub const DISPLAY: u32 = 30865;
pub const NAME: &str = "Jaina";
/// Under her name in the client (the creature's sub name).
pub const TITLE: &str = "Your companion";
/// HighGuid::Unit (0xF130), her entry, counter 1.
pub const GUID: u64 = (0xF130u64 << 48) | ((ENTRY as u64) << 24) | 1;
pub const IN_FILE: &str = "companion-in.json";
pub const OUT_FILE: &str = "companion-out.json";

/// Yards behind the player, a little to the left (ov_den_companion.lua C.FOLLOW, C.ANGLE).
pub const FOLLOW: f32 = 2.5;
pub const ANGLE: f32 = PI * 0.75;
/// Further than this (a jump across the room): she comes back to the player's side at once.
pub const FAR: f32 = 30.0;
/// "Jaina, ..." is heard within this many yards.
pub const HEAR: f32 = 10.0;
/// She doesn't move for less than this ("following loosely").
pub const SLACK: f32 = 2.0;
pub const RUN: f32 = 7.0;
pub const WALK: f32 = 2.5;
pub const ASK_EVERY: u64 = 4;
pub const ASKS_KEEP: usize = 20;
pub const LEN: usize = 200;
pub const SAY_EVERY: u64 = 3;
pub const LINE_AGE: u64 = 300;
pub const LISTENING: u64 = 600;
pub const BEAT: u64 = 10;
/// The words she never says (den.rs BRAND_WORDS).
pub const BRAND_WORDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "blizzard", "wow"];

/// A place and a facing (radians).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct At {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
}

fn dist(a: At, b: At) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// One line of plain text: no control characters, no `|` (the game's chat codes), single spaces, up to 200 letters.
pub fn clean(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_control() || c == '|' { ' ' } else { c }).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(LEN).collect()
}

/// A brand word in the line (whole words for "wow"), as den.rs `brand_word`.
pub fn branded(s: &str) -> bool {
    let low = s.to_lowercase();
    let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let joined = words.join(" ");
    BRAND_WORDS.iter().any(|b| if *b == "wow" { words.contains(&"wow") } else { joined.contains(b) || low.contains(b) })
}

/// "Jaina, <words>" (or "Jaina: ...", "Jaina! ..."): the words.
pub fn heard(text: &str) -> Option<String> {
    let t = text.trim_start();
    let head = t.get(..NAME.len())?;
    if !head.eq_ignore_ascii_case(NAME) {
        return None;
    }
    let rest = t[NAME.len()..].trim_start();
    let rest = rest.strip_prefix([',', ':', '!'])?.trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

/// Her spot by the player: FOLLOW yards away at ANGLE from the way they face.
pub fn spot(p: At) -> At {
    At { x: p.x + (p.o + ANGLE).cos() * FOLLOW, y: p.y + (p.o + ANGLE).sin() * FOLLOW, z: p.z, o: p.o }
}

fn read_json(p: &Path, max: u64) -> Option<Value> {
    if fs::metadata(p).ok()?.len() > max {
        return None;
    }
    serde_json::from_str(fs::read_to_string(p).ok()?.trim_start_matches('\u{feff}')).ok()
}

/// Written whole, then renamed (den.rs reads it at any moment).
pub fn write_json(dir: &Path, name: &str, v: &Value) -> bool {
    let _ = fs::create_dir_all(dir);
    let tmp = dir.join(format!(".{name}.tmp"));
    let Ok(text) = serde_json::to_string_pretty(v) else { return false };
    fs::write(&tmp, text + "\n").and_then(|_| fs::rename(&tmp, dir.join(name))).is_ok()
}

/// What an ask did.
#[derive(Debug, Clone, PartialEq)]
pub enum Asked {
    /// Written down (its number); she says the line.
    Noted(u64, &'static str),
    /// Too soon after the last one: she says the line, nothing is written.
    Busy(&'static str),
}

pub const THINKING: &str = "Let me think about that.";
pub const NOBODY: &str =
    "I've written that down. Nobody is reading my notes right now: open the Agents page in Leyline and ask your assistant to read what you told me.";
pub const HELLO: &str = "Hello! Tell me what you'd like to build: say \"Jaina, ...\" near me, or .den ask.";
pub const HERE: &str = "Right here.";
pub const BYE: &str = "I'll be close by. Call me when you need me.";

#[derive(Debug, Default)]
pub struct Companion {
    pub out: bool,
    /// She walks from `from` to `to`, leaving at `start` and arriving at `arrive` (milliseconds).
    from: At,
    to: At,
    start: u64,
    arrive: u64,
    pub asks: Vec<Value>,
    next_ask: u64,
    /// The last line number taken from companion-out.json (said, or too old to say).
    pub said: u64,
    queue: VecDeque<String>,
    last_say: u64,
    last_ask: u64,
    last_beat: u64,
    pub moves: u32,
}

impl Companion {
    /// After a restart: the asks and the last line taken come back from companion-in.json, so nothing is said twice.
    pub fn restore(dir: &Path) -> Companion {
        let mut c = Companion { next_ask: 1, ..Default::default() };
        let Some(v) = read_json(&dir.join(IN_FILE), 256 * 1024) else { return c };
        c.said = v.get("said").and_then(Value::as_u64).unwrap_or(0);
        for a in v.get("asks").and_then(Value::as_array).into_iter().flatten() {
            let (Some(n), Some(text)) = (a.get("n").and_then(Value::as_u64), a.get("text").and_then(Value::as_str)) else { continue };
            let mut o = json!({"n": n, "at": a.get("at").and_then(Value::as_u64).unwrap_or(0), "text": clean(text)});
            if let Some(w) = a.get("where").filter(|w| w.is_object()) {
                o["where"] = w.clone();
            }
            c.asks.push(o);
            c.next_ask = c.next_ask.max(n + 1);
        }
        let n = c.asks.len();
        c.asks.drain(..n.saturating_sub(ASKS_KEEP));
        c
    }

    /// Where she stands now (on her way, part of the way).
    pub fn pos(&self, ms: u64) -> At {
        if ms >= self.arrive || self.arrive <= self.start {
            return self.to;
        }
        let t = (ms.saturating_sub(self.start)) as f32 / (self.arrive - self.start) as f32;
        At { x: self.from.x + (self.to.x - self.from.x) * t, y: self.from.y + (self.to.y - self.from.y) * t, z: self.to.z, o: self.to.o }
    }

    /// She comes to the player's side (or, out already, steps back to it): where she stands.
    pub fn come(&mut self, p: At, ms: u64) -> At {
        let at = spot(p);
        self.out = true;
        (self.from, self.to, self.start, self.arrive) = (at, at, ms, ms);
        at
    }

    /// She goes; true when she was out.
    pub fn away(&mut self) -> bool {
        std::mem::replace(&mut self.out, false)
    }

    /// One step of following the player at `p`: a walk to start (from, to, milliseconds), or None while she is close
    /// enough (or already on her way there).
    pub fn follow(&mut self, p: At, ms: u64) -> Option<(At, At, u32)> {
        if !self.out {
            return None;
        }
        let target = spot(p);
        let now = self.pos(ms);
        if dist(self.to, target) <= SLACK || dist(now, target) <= SLACK {
            return None;
        }
        let d = dist(now, target);
        let speed = if d > 6.0 { RUN } else { WALK };
        let duration = ((d / speed * 1000.0) as u32).max(100);
        let o = (target.y - now.y).atan2(target.x - now.x).rem_euclid(2.0 * PI);
        let to = At { o, ..target };
        (self.from, self.to, self.start, self.arrive) = (now, to, ms, ms + duration as u64);
        self.moves += 1;
        Some((now, to, duration))
    }

    /// Someone read the asks lately (companion-out.json `read_at`).
    pub fn listening(dir: &Path, now: u64) -> bool {
        read_json(&dir.join(OUT_FILE), 256 * 1024).and_then(|v| v.get("read_at").and_then(Value::as_u64)).is_some_and(|t| now.abs_diff(t) <= LISTENING)
    }

    /// `.den ask <words>` or "Jaina, <words>": written down for the player's assistant. `at`: where the player
    /// stands, relative to the Den (yards from its middle, z from its floor, yaw in degrees). Err: a system line.
    pub fn ask(&mut self, dir: &Path, text: &str, at: Option<At>, now: u64) -> Result<Asked, String> {
        if !self.out {
            return Err("Call your companion first: .den companion".into());
        }
        let text = clean(text);
        if text.is_empty() {
            return Err("Usage: .den ask <your words>, or say \"Jaina, ...\" near her.".into());
        }
        if self.last_ask != 0 && now.saturating_sub(self.last_ask) < ASK_EVERY {
            return Ok(Asked::Busy("One moment, I'm still on the last one."));
        }
        self.last_ask = now;
        let n = self.next_ask.max(1);
        self.next_ask = n + 1;
        let mut a = json!({"n": n, "at": now, "text": text});
        if let Some(w) = at {
            let r1 = |v: f32| (v as f64 * 10.0).round() / 10.0;
            a["where"] = json!({"x": r1(w.x), "y": r1(w.y), "z": r1(w.z), "yaw": r1(w.o.to_degrees().rem_euclid(360.0))});
        }
        self.asks.push(a);
        let k = self.asks.len();
        self.asks.drain(..k.saturating_sub(ASKS_KEEP));
        Ok(Asked::Noted(n, if Self::listening(dir, now) { THINKING } else { NOBODY }))
    }

    /// The assistant's lines (companion-out.json) after the last one taken; the fresh ones wait to be said.
    pub fn read_lines(&mut self, dir: &Path, now: u64) {
        let Some(v) = read_json(&dir.join(OUT_FILE), 256 * 1024) else { return };
        for l in v.get("lines").and_then(Value::as_array).into_iter().flatten() {
            let Some(n) = l.get("n").and_then(Value::as_u64) else { continue };
            if n <= self.said {
                continue;
            }
            self.said = n;
            let fresh = l.get("at").and_then(Value::as_u64).is_some_and(|at| now.abs_diff(at) <= LINE_AGE);
            if let (true, Some(t)) = (fresh, l.get("text").and_then(Value::as_str)) {
                self.queue.push_back(t.to_string());
            }
        }
        while self.queue.len() > ASKS_KEEP {
            self.queue.pop_front();
        }
    }

    /// The next line she says now (out, and 3 seconds after the last); a line she won't say doesn't take a turn.
    pub fn next_line(&mut self, now: u64) -> Option<String> {
        if !self.out || now.saturating_sub(self.last_say) < SAY_EVERY {
            return None;
        }
        while let Some(t) = self.queue.pop_front() {
            let t = clean(&t);
            if !t.is_empty() && !branded(&t) {
                self.last_say = now;
                return Some(t);
            }
        }
        None
    }

    /// The heartbeat (companion-in.json): every BEAT seconds, or now. `owner` and `room` as the file has them.
    pub fn beat(&mut self, dir: &Path, now: u64, force: bool, owner: Value, room: Value) -> bool {
        if !force && now.saturating_sub(self.last_beat) < BEAT {
            return false;
        }
        self.last_beat = now;
        let v = json!({"format": "leyline-companion", "version": 1, "server": "den-lite", "at": now, "said": self.said,
            "asks": self.asks, "companion": self.out, "owner": owner, "room": room});
        write_json(dir, IN_FILE, &v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("den-lite-companion-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn words_for_her() {
        assert_eq!(heard("Jaina, a well by the door"), Some("a well by the door".into()));
        assert_eq!(heard("  jaina: hi"), Some("hi".into()));
        assert_eq!(heard("Jaina hi"), None);
        assert_eq!(heard("Jaina,"), None);
        assert_eq!(heard("Jainas, hi"), None);
        assert_eq!(clean("a |cffff0000red|r\nline"), "a cffff0000red r line");
        assert!(branded("Wow, look") && branded("the Lich King") && !branded("wowza"));
    }

    #[test]
    fn she_follows_loosely() {
        let mut c = Companion::default();
        let p = At { x: 100.0, y: 100.0, z: 10.0, o: 0.0 };
        assert_eq!(c.follow(p, 0), None, "not out");
        let at = c.come(p, 0);
        assert!((dist(at, p) - FOLLOW).abs() < 1e-4);
        assert_eq!(c.follow(p, 0), None, "at her spot");
        assert_eq!(c.follow(At { x: 101.0, ..p }, 100), None, "a step: she stays");
        let far = At { x: 120.0, ..p };
        let (from, to, ms) = c.follow(far, 1000).expect("a walk");
        assert_eq!(from, at);
        assert!((dist(to, spot(far)) < 1e-4) && ms > 2000 && ms < 4000, "{ms}");
        assert_eq!(c.follow(far, 1500), None, "on her way already");
        let mid = c.pos(1000 + ms as u64 / 2);
        assert!(mid.x > from.x && mid.x < to.x);
        assert_eq!(c.pos(10_000), to);
        assert!(c.away() && !c.away());
    }

    #[test]
    fn asks_and_lines_through_the_files() {
        let d = dir("files");
        let mut c = Companion::restore(&d);
        assert!(c.ask(&d, "hi", None, 100).unwrap_err().contains(".den companion"));
        c.come(At::default(), 0);
        let w = At { x: 3.14159, y: -2.0, z: 0.0, o: PI };
        assert_eq!(c.ask(&d, "a well |here|", Some(w), 100), Ok(Asked::Noted(1, NOBODY)));
        assert_eq!(c.ask(&d, "and a bench", None, 102), Ok(Asked::Busy("One moment, I'm still on the last one.")));
        write_json(&d, OUT_FILE, &json!({"format": "leyline-companion", "lines": [], "seen": 1, "read_at": 104}));
        assert_eq!(c.ask(&d, "and a bench", None, 105), Ok(Asked::Noted(2, THINKING)));
        assert_eq!(c.asks[0]["where"], json!({"x": 3.1, "y": -2.0, "z": 0.0, "yaw": 180.0}));
        assert_eq!(c.asks[0]["text"], "a well here");
        assert!(c.beat(&d, 105, true, json!({"online": true}), json!({"half": 8})));
        assert!(!c.beat(&d, 106, false, json!({}), json!({})), "every 10 seconds");
        // den_say's lines: the old one is skipped, the brand one is never said
        write_json(&d, OUT_FILE, &json!({"lines": [{"n": 1, "at": 1, "text": "old"}, {"n": 2, "at": 200, "text": "Wow!"},
            {"n": 3, "at": 200, "text": "A well, here?\nI'll draw one up."}], "seen": 2, "read_at": 200}));
        c.read_lines(&d, 400);
        assert_eq!(c.said, 3);
        assert_eq!(c.next_line(400), Some("A well, here? I'll draw one up.".into()));
        assert_eq!(c.next_line(410), None);
        // a restart: the asks and the last line taken come back
        let back = Companion::restore(&d);
        assert_eq!((back.said, back.asks.len(), back.next_ask), (0, 2, 3), "said was 0 at the last beat");
        let v: Value = serde_json::from_str(&fs::read_to_string(d.join(IN_FILE)).unwrap()).unwrap();
        assert_eq!((v["server"].clone(), v["companion"].clone(), v["asks"][1]["n"].clone()), (json!("den-lite"), json!(true), json!(2)));
        let _ = fs::remove_dir_all(&d);
    }
}
