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
/// At login, when she is there by default (Config::companion): one short line, then quiet.
pub const GREET: &str = "I'm with you. Say \"Jaina, tips\" for ideas, or \"Jaina, stay\" and \"Jaina, follow\".";
pub const HERE: &str = "Right here.";
pub const BYE: &str = "I'll be close by. Call me when you need me.";
pub const STAYING: &str = "I'll wait here.";
pub const FOLLOWING: &str = "Right behind you.";
pub const COMING: &str = "On my way.";
pub const BACK: &str = "Back with you.";
/// Once a session, when the Den is still empty a while after login and the player has said nothing to her.
pub const NUDGE: &str = "Want a start? Try .den place chair, or say \"Jaina, tips\".";
/// Seconds after login before the one nudge, and how quiet the player must have been with her.
pub const NUDGE_AFTER: u64 = 120;
pub const NUDGE_QUIET: u64 = 60;
/// Closer than this to the player, she steps back to her spot even when it is within SLACK (she never stands in the way).
pub const BLOCK: f32 = 1.2;

/// What she does between asks: walks behind the player, or waits where she is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    Follow,
    Stay,
}

/// The short things said to her ("Jaina, stay"): handled here, never written down for the assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Follow,
    Stay,
    Come,
    /// A tip (also "help me": the Den has nothing to fight, so help is a tip).
    Help,
    Away,
}

/// One of the short commands, as the whole line (so "stay by the door, add a bench" is an ask, not a command).
pub fn command(words: &str) -> Option<Cmd> {
    let low = words.to_lowercase();
    let t = low.trim().trim_end_matches(['.', '!', '?']).trim();
    let t = t.strip_prefix("please ").unwrap_or(t);
    let t = t.strip_suffix(" please").unwrap_or(t).trim_end_matches(',').trim();
    Some(match t {
        "follow" | "follow me" | "with me" | "come with me" | "let's go" | "lets go" | "heel" => Cmd::Follow,
        "stay" | "stay here" | "wait" | "wait here" | "stop" | "hold" | "stand still" => Cmd::Stay,
        "come" | "come here" | "here" | "to me" | "come back" | "over here" => Cmd::Come,
        "help" | "help me" | "tip" | "tips" | "a tip" | "hint" | "what next" | "what now" | "what should i do"
        | "what can i do" | "what do i do" | "what do i do now" | "ideas" => Cmd::Help,
        "go away" | "bye" | "goodbye" | "dismiss" | "leave" | "go home" => Cmd::Away,
        _ => return None,
    })
}

/// Built-in answers, so she is a guide with no assistant connected: (any of these words, the answer). The first
/// entry whose word is in the ask wins; deterministic, no network. TODO(owner): a voice pass on the lines.
pub const GUIDE: &[(&[&str], &str)] = &[
    (&["place", "put", "build", "chair", "table", "lamp", "furniture"], "Type .den place and a name, like .den place chair. .den find lamp shows what there is."),
    (&["stamp", "stamps", "inn", "house", "camp", "campfire"], "Stamps are whole builds: .den stamps lists them, .den stamp inn now puts one where you stand."),
    (&["undo", "mistake", "wrong", "remove", "delete"], ".build undo takes the last stamp or load back. It remembers 50 steps."),
    (&["save", "keep", "file"], ".den save and a name keeps your Den in a file. The launcher's Saved Dens opens it again."),
    (&["load", "open", "files", "saved"], ".den files lists your saved Dens; .den load and a name opens one (a copy of what was there is kept)."),
    (&["grow", "bigger", "size", "room", "space", "small"], ".den grow makes the room bigger, up to size 4. .den room stone or wood changes its look."),
    (&["look", "white", "stone", "wood", "theme", "color", "colour"], ".den room white, stone or wood changes the room's look."),
    (&["addon", "addons", "mod", "mods", "interface", "ui"], ".den mods shows the addons your game loaded. Add more in the launcher's Mods page; your Den is a quiet place to try them."),
    (&["mode", "modes", "minigolf", "game"], ".den mode lists game modes; .den mode and an id says how to run one on this PC."),
    (&["ground", "terrain", "hill", "hills", "pond", "trees", "forge"], "To shape the ground, open Your Den in the launcher and press Open in Forge. Save and close it to bring your changes in."),
    (&["friend", "friends", "invite", "together", "share"], "To build with friends, press Invite friends on the launcher's Your Den page."),
    (&["level", "levelling", "leveling", "quest", "quests", "fight", "dungeon", "dungeons", "xp"], "Your Den is for building. To level with helper bots, pick Solo leveling in the launcher (it needs Docker)."),
    (&["claude", "assistant", "ai", "agent"], "Connect your own assistant once on the launcher's Agents page. Then what you tell me reaches it, and it can build with you after your OK."),
    (&["follow", "stay", "come", "wait"], "Say \"Jaina, stay\" and I wait; \"Jaina, follow\" and I come along; \"Jaina, come\" and I walk over."),
];

/// The built-in answer for an ask, if one fits (whole words).
pub fn guide(text: &str) -> Option<&'static str> {
    let low = text.to_lowercase();
    let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    GUIDE.iter().find(|(keys, _)| keys.iter().any(|k| words.contains(k))).map(|(_, a)| *a)
}

/// What the tips look at: the Den as it is now.
#[derive(Debug, Clone, Copy, Default)]
pub struct TipCtx {
    pub props: usize,
    /// A Den file saved in the Dens folder (other than `backup`).
    pub saved: bool,
    /// 1 to 4.
    pub size: u32,
}

/// The tips after the first steps are done, in turn.
pub const TIPS: &[&str] = &[
    "Try a whole build: .den stamp inn now.",
    ".den room stone gives the room another look.",
    "Turn to face where a piece should look, then place it: it faces the way you face.",
    ".den mods shows your addons. Your Den is a quiet place to try new ones.",
    "Say \"Jaina, \" and what you'd like built: your assistant reads it once it is connected on the Agents page.",
    "Press Invite friends in the launcher to build together.",
];

fn tip_for(ctx: TipCtx, n: usize) -> &'static str {
    if ctx.props == 0 {
        return "Start with one piece: .den place chair puts a chair in front of you.";
    }
    if !ctx.saved {
        return "Keep it safe: .den save mine writes your Den to a file.";
    }
    if ctx.size <= 1 && ctx.props >= 8 {
        return "Running out of room? .den grow makes it bigger.";
    }
    TIPS[n % TIPS.len()]
}

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
    pub order: Order,
    /// She is wanted by the player's side (not sent away): she comes back by herself when they step into the Den again.
    pub wanted: bool,
    /// The session: when the player came into the world, the last time they said anything to her, the nudge said.
    arrived: u64,
    last_heard: u64,
    nudged: bool,
    tip_n: usize,
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
        (self.out, self.wanted, self.order) = (true, true, Order::Follow);
        (self.from, self.to, self.start, self.arrive) = (at, at, ms, ms);
        at
    }

    /// She goes out of sight (the player left the Den, or logged out); true when she was out. Still wanted: she
    /// comes back when they are in the Den again.
    pub fn away(&mut self) -> bool {
        std::mem::replace(&mut self.out, false)
    }

    /// The player sent her off: she stays away until called.
    pub fn dismiss(&mut self) -> bool {
        self.wanted = false;
        self.away()
    }

    /// A new session in the world: the nudge may be said once again, quiet since now.
    pub fn arrive(&mut self, now: u64) {
        (self.arrived, self.last_heard, self.nudged) = (now, now, false);
    }

    /// The player said something to her (a command or an ask): no nudge for a while.
    pub fn heard_now(&mut self, now: u64) {
        self.last_heard = now;
    }

    /// The one nudge of a session: the Den still empty NUDGE_AFTER seconds after login, quiet with her for NUDGE_QUIET.
    pub fn nudge(&mut self, now: u64, props: usize) -> Option<&'static str> {
        if !self.out || self.nudged || props > 0 || self.arrived == 0 {
            return None;
        }
        if now.saturating_sub(self.arrived) < NUDGE_AFTER || now.saturating_sub(self.last_heard) < NUDGE_QUIET {
            return None;
        }
        self.nudged = true;
        Some(NUDGE)
    }

    /// "Jaina, tips": the next step that fits the Den, then the general tips in turn.
    pub fn tip(&mut self, ctx: TipCtx) -> &'static str {
        let t = tip_for(ctx, self.tip_n);
        if std::ptr::eq(t, TIPS[self.tip_n % TIPS.len()]) {
            self.tip_n += 1;
        }
        t
    }

    /// "Jaina, stay": she waits where she is (stops on the spot she has reached).
    pub fn stay(&mut self, ms: u64) -> At {
        let here = self.pos(ms);
        self.order = Order::Stay;
        (self.from, self.to, self.start, self.arrive) = (here, here, ms, ms);
        here
    }

    /// "Jaina, follow": back to following (the next tick walks her over when she is away from her spot).
    pub fn follow_on(&mut self) {
        self.order = Order::Follow;
    }

    /// "Jaina, come": she walks to her spot by the player now, and follows again.
    pub fn come_walk(&mut self, p: At, ms: u64) -> Option<(At, At, u32)> {
        if !self.out {
            return None;
        }
        self.order = Order::Follow;
        let now = self.pos(ms);
        if dist(now, spot(p)) <= 0.5 {
            return None;
        }
        Some(self.walk_to(now, spot(p), ms))
    }

    fn walk_to(&mut self, now: At, target: At, ms: u64) -> (At, At, u32) {
        let d = dist(now, target);
        let speed = if d > 6.0 { RUN } else { WALK };
        let duration = ((d / speed * 1000.0) as u32).max(100);
        let o = (target.y - now.y).atan2(target.x - now.x).rem_euclid(2.0 * PI);
        let to = At { o, ..target };
        (self.from, self.to, self.start, self.arrive) = (now, to, ms, ms + duration as u64);
        self.moves += 1;
        (now, to, duration)
    }

    /// One step of following the player at `p`: a walk to start (from, to, milliseconds), or None while she is close
    /// enough (or already on her way there).
    pub fn follow(&mut self, p: At, ms: u64) -> Option<(At, At, u32)> {
        if !self.out || self.order == Order::Stay {
            return None;
        }
        let target = spot(p);
        let now = self.pos(ms);
        let walking = ms < self.arrive;
        // in the player's way (they stepped onto her): she steps back to her spot, however close it is
        let blocking = !walking && dist(now, p) < BLOCK && dist(now, target) > 0.3;
        if !blocking && (dist(self.to, target) <= SLACK || dist(now, target) <= SLACK) {
            return None;
        }
        Some(self.walk_to(now, target, ms))
    }

    /// Following and too far from the player (a jump, a teleport): she should be put back at their side at once.
    pub fn too_far(&self, p: At, ms: u64) -> bool {
        self.out && self.order == Order::Follow && dist(self.pos(ms), p) > FAR
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
        self.last_heard = now;
        let line = if Self::listening(dir, now) { THINKING } else { guide(&text).unwrap_or(NOBODY) };
        Ok(Asked::Noted(n, line))
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

    #[test]
    fn short_commands_are_hers_and_everything_else_is_an_ask() {
        assert_eq!(command("stay"), Some(Cmd::Stay));
        assert_eq!(command(" Wait here! "), Some(Cmd::Stay));
        assert_eq!(command("please follow me"), Some(Cmd::Follow));
        assert_eq!(command("come here, please"), Some(Cmd::Come));
        assert_eq!(command("Help me"), Some(Cmd::Help));
        assert_eq!(command("what next?"), Some(Cmd::Help));
        assert_eq!(command("go away"), Some(Cmd::Away));
        assert_eq!(command("stay by the door and add a bench"), None);
        assert_eq!(command("a well by the door"), None);
    }

    #[test]
    fn she_stays_comes_and_never_stands_in_the_way() {
        let mut c = Companion::default();
        let p = At { x: 0.0, y: 0.0, z: 0.0, o: 0.0 };
        let at = c.come(p, 0);
        // stay: the player runs off, she doesn't follow, and isn't pulled back across the room either
        assert_eq!(c.stay(0), at);
        let far = At { x: 40.0, ..p };
        assert_eq!(c.follow(far, 1000), None);
        assert!(!c.too_far(far, 1000), "staying: far is fine");
        // come: she walks over (not a jump) and follows again
        let (from, to, ms) = c.come_walk(far, 2000).expect("a walk");
        assert_eq!(from, at);
        assert!(dist(to, spot(far)) < 1e-4 && ms > 4000, "{ms}");
        assert_eq!(c.order, Order::Follow);
        let there = c.pos(2000 + ms as u64);
        // following, a jump: too far, put back at the side at once
        assert!(c.too_far(At { x: 100.0, ..p }, 2000 + ms as u64));
        // the player steps onto her (1 yard from her, her spot only 1.6 yards off): she steps aside
        let onto = At { x: there.x + 1.0, y: there.y, z: 0.0, o: 0.0 };
        assert!(dist(spot(onto), there) <= SLACK, "within the slack, so only the blocking rule moves her");
        let (_, to, _) = c.follow(onto, 3000 + ms as u64).expect("she steps out of the way");
        assert!(dist(to, spot(onto)) < 1e-4);
        assert!(c.dismiss() && !c.wanted);
    }

    #[test]
    fn a_guide_without_an_assistant_and_tips_that_fit_the_den() {
        assert!(guide("how do I save this?").unwrap().starts_with(".den save"));
        assert!(guide("can I make the room bigger").unwrap().starts_with(".den grow"));
        assert!(guide("where do I level up? quests?").unwrap().contains("Solo leveling"));
        assert!(guide("my addons").unwrap().starts_with(".den mods"));
        assert_eq!(guide("a well by the door"), None);
        for (_, a) in GUIDE {
            assert!(!branded(a) && a.len() <= LEN, "{a}");
        }
        let d = dir("guide");
        let mut c = Companion::restore(&d);
        c.come(At::default(), 0);
        assert_eq!(c.ask(&d, "how do I save?", None, 100), Ok(Asked::Noted(1, GUIDE[3].1)), "nobody listening: the built-in answer");
        write_json(&d, OUT_FILE, &json!({"lines": [], "read_at": 200}));
        assert_eq!(c.ask(&d, "how do I save?", None, 200), Ok(Asked::Noted(2, THINKING)), "the assistant is listening: it answers");
        // tips: the next step first, then the general ones in turn
        assert!(c.tip(TipCtx { props: 0, saved: false, size: 1 }).contains(".den place chair"));
        assert!(c.tip(TipCtx { props: 3, saved: false, size: 1 }).contains(".den save"));
        assert!(c.tip(TipCtx { props: 9, saved: true, size: 1 }).contains(".den grow"));
        let a = c.tip(TipCtx { props: 3, saved: true, size: 2 });
        let b = c.tip(TipCtx { props: 3, saved: true, size: 2 });
        assert_eq!((a, b), (TIPS[0], TIPS[1]));
        for t in TIPS {
            assert!(!branded(t));
        }
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn one_nudge_a_session_and_only_into_silence() {
        let mut c = Companion::default();
        c.come(At::default(), 0);
        c.arrive(1000);
        assert_eq!(c.nudge(1000 + NUDGE_AFTER - 1, 0), None, "too soon");
        c.heard_now(1000 + NUDGE_AFTER - 10);
        assert_eq!(c.nudge(1000 + NUDGE_AFTER, 0), None, "the player just spoke to her");
        assert_eq!(c.nudge(1000 + NUDGE_AFTER + NUDGE_QUIET, 3), None, "the Den has pieces");
        assert_eq!(c.nudge(1000 + NUDGE_AFTER + NUDGE_QUIET, 0), Some(NUDGE));
        assert_eq!(c.nudge(5000, 0), None, "once");
        c.arrive(9000);
        assert_eq!(c.nudge(9000 + NUDGE_AFTER + NUDGE_QUIET, 0), Some(NUDGE), "a new session");
    }
}
