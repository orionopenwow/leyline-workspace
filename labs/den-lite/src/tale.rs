//! Den Tales: small questlines a player writes (or asks Jaina for) and plays in their Den (docs/DEN-NEXT.md, "The tale
//! script"). A tale is plain text, one statement a line, readable aloud and short enough to pass around as a code:
//!
//! ```text
//! tale The Lost Lantern
//! by Wren
//! npc mira "Mira" look villager at 4 2
//! item lantern "Mira's Lantern" prop lantern at 12 -6
//! step Find Mira's lantern
//!   say mira "My lantern rolled off into the grass. Would you find it?"
//!   goal collect lantern
//!   hint It glows near the rocks.
//! step Bring it back
//!   goal talk mira
//!   done mira "You found it! Thank you."
//! when enter 0 0 6 do say "The old oak hums as you pass."
//! reward title Lantern Finder
//! ```
//!
//! Nothing in a tale is ever run as code: it is parsed into the [`Tale`] below, checked ([`parse`]: every id known,
//! every number in range, sizes capped, no links, no brand words), and played by [`Run`], a small state machine fed
//! with what happened ([`Ev`]: the player is near a spot, a dummy went down, time passed) that answers with what to
//! show ([`Act`]). Rewards are cosmetic only (a title, a cheer, a glow): nothing a tale does changes a character.
//!
//! Std only (no crate of the Den), so `rustc --test src/tale.rs` checks it on any machine.

use std::fmt::Write as _;

pub const FORMAT: &str = "tale";
/// The whole text, at most (a code carries it compressed).
pub const MAX_BYTES: usize = 8 * 1024;
pub const MAX_STEPS: usize = 24;
pub const MAX_THINGS: usize = 24;
pub const MAX_WHENS: usize = 24;
pub const MAX_REWARDS: usize = 4;
/// Lines said or shown per step (its `say`, `done`, `hint`).
pub const MAX_STEP_LINES: usize = 6;
pub const MAX_TEXT: usize = 160;
pub const MAX_TITLE: usize = 48;
/// How far from the Den's middle a tale may put anything (yards; the biggest Den is larger, the island 60).
pub const MAX_XY: f32 = 200.0;
/// Close enough to talk to someone, pick something up, read a sign (yards, on the flat).
pub const TALK_YARDS: f32 = 4.0;
pub const PICK_YARDS: f32 = 2.5;
pub const READ_YARDS: f32 = 3.0;
pub const PORTAL_YARDS: f32 = 1.8;
/// The words a tale may never show (the game's brand; companion.rs BRAND_WORDS, the launcher's too).
pub const BRAND_WORDS: &[&str] = &["world of warcraft", "warcraft", "lich king", "blizzard", "wow"];
/// The looks an NPC may have (the glue maps each to one of the game's own creature displays).
pub const LOOKS: &[&str] = &["villager", "guard", "mage", "dwarf", "elf", "ghost", "dummy", "jaina"];

/// Jaina herself: `jaina` is always an NPC (the companion), never declared.
pub const JAINA: &str = "jaina";

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Someone to talk to: a name and a look.
    Npc { name: String, look: String },
    /// Something to pick up: a name and the prop it is shown as (a catalog word, `.den find`).
    Item { name: String, prop: String },
    /// A sign: its words show when you walk up to it.
    Sign { text: String },
    /// A teleporter: step on it, you are at `to`.
    Portal { to: [f32; 2] },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Thing {
    pub id: String,
    pub at: [f32; 2],
    /// Degrees.
    pub face: f32,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Talk(String),
    Collect(String),
    /// n dummies brought down (a training dummy refills: each refill is one).
    Defeat(u32),
    Reach { at: [f32; 2], r: f32 },
    /// Walk up to a sign, an item or an NPC.
    Visit(String),
    /// Seconds.
    Wait(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Who says it (an NPC id, `jaina`), or None: the narrator (a system line).
    pub who: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub title: String,
    pub goal: Goal,
    pub say: Vec<Line>,
    pub done: Vec<Line>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    Say(Line),
    Teleport([f32; 2]),
    Show(String),
    Hide(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum When {
    Enter { at: [f32; 2], r: f32 },
    /// Walk up to a thing (any kind).
    Near(String),
    /// Seconds after the tale started.
    Timer(u32),
    /// Step n (1-based) starts.
    Step(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub when: When,
    pub act: Do,
    /// Fires again after the player left (enter / near only); otherwise once.
    pub again: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reward {
    Title(String),
    Badge(String),
    Cheer,
    Glow,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tale {
    pub title: String,
    pub by: Option<String>,
    pub about: Option<String>,
    pub things: Vec<Thing>,
    pub steps: Vec<Step>,
    pub whens: Vec<Trigger>,
    pub rewards: Vec<Reward>,
}

/// A problem in a tale's text: the line (1-based, 0: the whole tale) and what is wrong, in plain words.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub what: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.what)
        } else {
            write!(f, "line {}: {}", self.line, self.what)
        }
    }
}

fn bad(line: usize, what: impl Into<String>) -> Problem {
    Problem { line, what: what.into() }
}

/// The words of one line: bare words, and "quoted text" as one (a `\"` inside is a quote). A `#` outside quotes
/// starts a comment.
pub fn words(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut in_word = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            match c {
                '\\' if it.peek() == Some(&'"') => {
                    cur.push('"');
                    it.next();
                }
                '"' => {
                    quoted = false;
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
                _ => cur.push(c),
            }
            continue;
        }
        match c {
            '#' => break,
            '"' => {
                if in_word {
                    return Err("a quote in the middle of a word".into());
                }
                quoted = true;
            }
            c if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            c => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if quoted {
        return Err("a quote is never closed".into());
    }
    if in_word {
        out.push(cur);
    }
    Ok(out)
}

/// Text a player sees: no control characters, no links, no brand words, not empty, at most `max` letters.
pub fn clean_text(s: &str, max: usize) -> Result<String, String> {
    let t: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("empty text".into());
    }
    if t.chars().count() > max {
        return Err(format!("text longer than {max} letters"));
    }
    let low = t.to_lowercase();
    if low.contains("http") || low.contains("www.") || low.contains("://") || low.contains(".com") || low.contains(".gg") {
        return Err("no links in a tale".into());
    }
    if branded(&t) {
        return Err("that name can't be shown in Leyline".into());
    }
    Ok(t)
}

/// A brand word (whole word for "wow").
pub fn branded(s: &str) -> bool {
    let low = s.to_lowercase();
    let ws: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let joined = ws.join(" ");
    BRAND_WORDS.iter().any(|b| if *b == "wow" { ws.contains(&"wow") } else { joined.contains(b) })
}

/// An id: 1 to 16 of a-z, 0-9, - and _, starting with a letter.
pub fn id_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 16
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn num(s: &str, lo: f32, hi: f32) -> Option<f32> {
    let v: f32 = s.parse().ok()?;
    (v.is_finite() && v >= lo && v <= hi).then_some(v)
}

fn count(s: &str, lo: u32, hi: u32) -> Option<u32> {
    let v: u32 = s.parse().ok()?;
    (lo..=hi).contains(&v).then_some(v)
}

/// `at <x> <y>` at `w[i..]`.
fn at_of(w: &[String], i: usize, line: usize) -> Result<[f32; 2], Problem> {
    if w.get(i).map(|s| s.to_ascii_lowercase()) != Some("at".into()) {
        return Err(bad(line, "expected: at <x> <y>"));
    }
    xy_of(w, i + 1, line)
}

fn xy_of(w: &[String], i: usize, line: usize) -> Result<[f32; 2], Problem> {
    let x = w.get(i).and_then(|s| num(s, -MAX_XY, MAX_XY));
    let y = w.get(i + 1).and_then(|s| num(s, -MAX_XY, MAX_XY));
    match (x, y) {
        (Some(x), Some(y)) => Ok([x, y]),
        _ => Err(bad(line, format!("a place is two numbers from -{MAX_XY} to {MAX_XY} (yards from the Den's middle)"))),
    }
}

/// `<npc> "text"` or `"text"` (the narrator).
fn line_of(w: &[String], i: usize, line: usize) -> Result<Line, Problem> {
    match (w.get(i), w.get(i + 1)) {
        (Some(who), Some(t)) if w.len() == i + 2 => {
            let who = who.to_ascii_lowercase();
            if !id_ok(&who) {
                return Err(bad(line, "who says it: an npc's id"));
            }
            Ok(Line { who: Some(who), text: clean_text(t, MAX_TEXT).map_err(|e| bad(line, e))? })
        }
        (Some(t), None) => Ok(Line { who: None, text: clean_text(t, MAX_TEXT).map_err(|e| bad(line, e))? }),
        _ => Err(bad(line, "expected: say [who] \"text\"")),
    }
}

/// The rest of the line as text (a title, a hint): bare words or one quoted.
fn rest_text(w: &[String], i: usize, max: usize, line: usize) -> Result<String, Problem> {
    let t = w.get(i..).map(|r| r.join(" ")).unwrap_or_default();
    clean_text(&t, max).map_err(|e| bad(line, e))
}

fn face_of(w: &[String], i: usize, line: usize) -> Result<f32, Problem> {
    match (w.get(i).map(|s| s.to_ascii_lowercase()), w.get(i + 1)) {
        (None, _) => Ok(0.0),
        (Some(f), Some(v)) if f == "face" && w.len() == i + 2 => num(v, -360.0, 360.0).ok_or_else(|| bad(line, "face: degrees, -360 to 360")),
        _ => Err(bad(line, "unexpected words at the end (only: face <degrees>)")),
    }
}

fn goal_of(w: &[String], line: usize) -> Result<Goal, Problem> {
    let k = w.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let id = |i: usize| -> Result<String, Problem> {
        let mut v = w.get(i).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        if v == "companion" {
            v = JAINA.to_string(); // the launcher's word for her (its tool text never names her)
        }
        if id_ok(&v) { Ok(v) } else { Err(bad(line, format!("goal {k}: an id"))) }
    };
    let g = match k.as_str() {
        "talk" => Goal::Talk(id(2)?),
        "collect" => Goal::Collect(id(2)?),
        "visit" => Goal::Visit(id(2)?),
        "defeat" => {
            let n = w.get(2).and_then(|s| count(s, 1, 50)).ok_or_else(|| bad(line, "goal defeat: 1 to 50 (dummies)"))?;
            if let Some(what) = w.get(3) {
                if !matches!(what.to_ascii_lowercase().as_str(), "dummy" | "dummies") || w.len() > 4 {
                    return Err(bad(line, "goal defeat <n> dummies"));
                }
            }
            return Ok(Goal::Defeat(n));
        }
        "reach" => {
            let at = xy_of(w, 2, line)?;
            let r = match w.get(4) {
                None => 3.0,
                Some(r) => num(r, 1.0, 30.0).ok_or_else(|| bad(line, "goal reach: a radius from 1 to 30"))?,
            };
            if w.len() > 5 {
                return Err(bad(line, "goal reach <x> <y> [radius]"));
            }
            return Ok(Goal::Reach { at, r });
        }
        "wait" => Goal::Wait(w.get(2).and_then(|s| count(s, 1, 600)).ok_or_else(|| bad(line, "goal wait: 1 to 600 seconds"))?),
        _ => return Err(bad(line, "goal: talk, collect, visit, defeat, reach or wait")),
    };
    if w.len() > 3 {
        return Err(bad(line, "unexpected words after the goal"));
    }
    Ok(g)
}

fn do_of(w: &[String], i: usize, line: usize) -> Result<Do, Problem> {
    let k = w.get(i).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    match k.as_str() {
        "say" => Ok(Do::Say(line_of(w, i + 1, line)?)),
        "teleport" => {
            let at = xy_of(w, i + 1, line)?;
            if w.len() > i + 3 {
                return Err(bad(line, "teleport <x> <y>"));
            }
            Ok(Do::Teleport(at))
        }
        "show" | "hide" => {
            let id = w.get(i + 1).map(|s| s.to_ascii_lowercase()).filter(|s| id_ok(s)).ok_or_else(|| bad(line, format!("{k} <id>")))?;
            if w.len() > i + 2 {
                return Err(bad(line, format!("{k} <id>")));
            }
            Ok(if k == "show" { Do::Show(id) } else { Do::Hide(id) })
        }
        _ => Err(bad(line, "do: say, teleport, show or hide")),
    }
}

fn when_of(w: &[String], line: usize) -> Result<Trigger, Problem> {
    let k = w.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let mut words: Vec<String> = w.to_vec();
    let again = words.last().is_some_and(|l| l.eq_ignore_ascii_case("again"));
    if again {
        words.pop();
    }
    let d = words.iter().position(|x| x.eq_ignore_ascii_case("do")).ok_or_else(|| bad(line, "when ... do <what>"))?;
    let (head, _) = words.split_at(d);
    let when = match k.as_str() {
        "enter" if head.len() == 5 => {
            let at = xy_of(head, 2, line)?;
            let r = num(&head[4], 1.0, 30.0).ok_or_else(|| bad(line, "when enter <x> <y> <radius 1-30>"))?;
            When::Enter { at, r }
        }
        "near" if head.len() == 3 && id_ok(&head[2].to_ascii_lowercase()) => When::Near(head[2].to_ascii_lowercase()),
        "timer" if head.len() == 3 => When::Timer(count(&head[2], 5, 3600).ok_or_else(|| bad(line, "when timer <5-3600 seconds>"))?),
        "step" if head.len() == 3 => When::Step(count(&head[2], 1, MAX_STEPS as u32).ok_or_else(|| bad(line, "when step <n>"))? as usize),
        _ => return Err(bad(line, "when enter <x> <y> <r> | near <id> | timer <seconds> | step <n>, then do <what>")),
    };
    if again && !matches!(when, When::Enter { .. } | When::Near(_)) {
        return Err(bad(line, "again: only for enter and near"));
    }
    Ok(Trigger { when, act: do_of(&words, d + 1, line)?, again })
}

/// Reads and checks a tale. Every problem is a [`Problem`] naming its line.
pub fn parse(text: &str) -> Result<Tale, Problem> {
    let text = text.trim_start_matches('\u{feff}');
    if text.len() > MAX_BYTES {
        return Err(bad(0, format!("a tale is at most {} KB", MAX_BYTES / 1024)));
    }
    let mut t = Tale::default();
    let mut seen_title = false;
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let w = words(raw).map_err(|e| bad(n, e))?;
        let Some(head) = w.first().map(|s| s.to_ascii_lowercase()) else { continue };
        if !seen_title && head != FORMAT {
            return Err(bad(n, "a tale starts with: tale <its title>"));
        }
        match head.as_str() {
            "tale" => {
                if seen_title {
                    return Err(bad(n, "one tale per text"));
                }
                t.title = rest_text(&w, 1, MAX_TITLE, n)?;
                seen_title = true;
            }
            "by" => t.by = Some(rest_text(&w, 1, 24, n)?),
            "about" => t.about = Some(rest_text(&w, 1, MAX_TEXT, n)?),
            "npc" | "item" | "sign" | "portal" => {
                if t.things.len() >= MAX_THINGS {
                    return Err(bad(n, format!("at most {MAX_THINGS} npcs, items, signs and portals")));
                }
                let id = w.get(1).map(|s| s.to_ascii_lowercase()).filter(|s| id_ok(s)).ok_or_else(|| bad(n, format!("{head} <id>: a-z, 0-9, - and _ (16 at most)")))?;
                if id == JAINA {
                    return Err(bad(n, "jaina is always here: talk to her without declaring her"));
                }
                if t.things.iter().any(|x| x.id == id) {
                    return Err(bad(n, format!("{id} is already used")));
                }
                let thing = match head.as_str() {
                    // npc <id> "<Name>" look <look> at <x> <y> [face <deg>]
                    "npc" => {
                        let name = clean_text(w.get(2).map(String::as_str).unwrap_or(""), 24).map_err(|e| bad(n, format!("npc name: {e}")))?;
                        if w.get(3).map(|s| s.to_ascii_lowercase()) != Some("look".into()) {
                            return Err(bad(n, "npc <id> \"<Name>\" look <look> at <x> <y>"));
                        }
                        let look = w.get(4).map(|s| s.to_ascii_lowercase()).filter(|l| LOOKS.contains(&l.as_str())).ok_or_else(|| bad(n, format!("look: {}", LOOKS.join(", "))))?;
                        let at = at_of(&w, 5, n)?;
                        Thing { id, at, face: face_of(&w, 8, n)?, kind: Kind::Npc { name, look } }
                    }
                    // item <id> "<Name>" prop <catalog word> at <x> <y>
                    "item" => {
                        let name = clean_text(w.get(2).map(String::as_str).unwrap_or(""), 32).map_err(|e| bad(n, format!("item name: {e}")))?;
                        if w.get(3).map(|s| s.to_ascii_lowercase()) != Some("prop".into()) {
                            return Err(bad(n, "item <id> \"<Name>\" prop <what it looks like> at <x> <y>"));
                        }
                        let prop = w.get(4).map(|s| s.to_ascii_lowercase()).filter(|p| p.len() <= 24 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).ok_or_else(|| bad(n, "prop: one word from the catalog (.den find)"))?;
                        let at = at_of(&w, 5, n)?;
                        Thing { id, at, face: face_of(&w, 8, n)?, kind: Kind::Item { name, prop } }
                    }
                    // sign <id> at <x> <y> "<text>"
                    "sign" => {
                        let at = at_of(&w, 2, n)?;
                        let text = clean_text(w.get(5).map(String::as_str).unwrap_or(""), MAX_TEXT).map_err(|e| bad(n, format!("sign: {e}")))?;
                        if w.len() > 6 {
                            return Err(bad(n, "sign <id> at <x> <y> \"<text>\""));
                        }
                        Thing { id, at, face: 0.0, kind: Kind::Sign { text } }
                    }
                    // portal <id> at <x> <y> to <x> <y>
                    _ => {
                        let at = at_of(&w, 2, n)?;
                        if w.get(5).map(|s| s.to_ascii_lowercase()) != Some("to".into()) || w.len() != 8 {
                            return Err(bad(n, "portal <id> at <x> <y> to <x> <y>"));
                        }
                        let to = xy_of(&w, 6, n)?;
                        Thing { id, at, face: 0.0, kind: Kind::Portal { to } }
                    }
                };
                t.things.push(thing);
            }
            "step" => {
                if t.steps.len() >= MAX_STEPS {
                    return Err(bad(n, format!("at most {MAX_STEPS} steps")));
                }
                // the goal comes on its own line; a placeholder until then
                t.steps.push(Step { title: rest_text(&w, 1, MAX_TITLE, n)?, goal: Goal::Wait(0), say: Vec::new(), done: Vec::new(), hint: None });
            }
            "goal" | "say" | "done" | "hint" => {
                let Some(s) = t.steps.last_mut() else { return Err(bad(n, format!("{head} belongs to a step: step <title> first"))) };
                let lines = s.say.len() + s.done.len() + s.hint.is_some() as usize;
                match head.as_str() {
                    "goal" => {
                        if s.goal != Goal::Wait(0) {
                            return Err(bad(n, "one goal per step"));
                        }
                        s.goal = goal_of(&w, n)?;
                    }
                    _ if lines >= MAX_STEP_LINES => return Err(bad(n, format!("at most {MAX_STEP_LINES} lines in a step"))),
                    "say" => s.say.push(line_of(&w, 1, n)?),
                    "done" => s.done.push(line_of(&w, 1, n)?),
                    _ => {
                        if s.hint.is_some() {
                            return Err(bad(n, "one hint per step"));
                        }
                        s.hint = Some(rest_text(&w, 1, MAX_TEXT, n)?);
                    }
                }
            }
            "when" => {
                if t.whens.len() >= MAX_WHENS {
                    return Err(bad(n, format!("at most {MAX_WHENS} whens")));
                }
                t.whens.push(when_of(&w, n)?);
            }
            "reward" => {
                if t.rewards.len() >= MAX_REWARDS {
                    return Err(bad(n, format!("at most {MAX_REWARDS} rewards")));
                }
                let k = w.get(1).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
                let r = match k.as_str() {
                    "title" => Reward::Title(rest_text(&w, 2, 32, n)?),
                    "badge" => Reward::Badge(rest_text(&w, 2, 32, n)?),
                    "cheer" if w.len() == 2 => Reward::Cheer,
                    "glow" if w.len() == 2 => Reward::Glow,
                    _ => return Err(bad(n, "reward title <text> | badge <text> | cheer | glow (looks only, never gear or gold)")),
                };
                t.rewards.push(r);
            }
            _ => return Err(bad(n, format!("unknown word '{head}' (tale, by, about, npc, item, sign, portal, step, goal, say, done, hint, when, reward)"))),
        }
    }
    if !seen_title {
        return Err(bad(0, "a tale starts with: tale <its title>"));
    }
    if t.steps.is_empty() {
        return Err(bad(0, "a tale needs at least one step"));
    }
    check_refs(&t)?;
    Ok(t)
}

fn check_refs(t: &Tale) -> Result<(), Problem> {
    let kind = |id: &str| t.things.iter().find(|x| x.id == id).map(|x| &x.kind);
    let speaker = |who: &Option<String>| match who {
        None => true,
        Some(w) if w == JAINA => true,
        Some(w) => matches!(kind(w), Some(Kind::Npc { .. })),
    };
    for (i, s) in t.steps.iter().enumerate() {
        let what = |e: &str| bad(0, format!("step {} ({}): {e}", i + 1, s.title));
        match &s.goal {
            Goal::Wait(0) => return Err(what("no goal (goal talk|collect|visit|defeat|reach|wait ...)")),
            Goal::Talk(id) if id != JAINA && !matches!(kind(id), Some(Kind::Npc { .. })) => return Err(what(&format!("talk {id}: no such npc"))),
            Goal::Collect(id) if !matches!(kind(id), Some(Kind::Item { .. })) => return Err(what(&format!("collect {id}: no such item"))),
            Goal::Visit(id) if id != JAINA && kind(id).is_none() => return Err(what(&format!("visit {id}: nothing called that"))),
            _ => {}
        }
        if let Some(l) = s.say.iter().chain(s.done.iter()).find(|l| !speaker(&l.who)) {
            return Err(what(&format!("{} isn't an npc", l.who.clone().unwrap_or_default())));
        }
    }
    for w in &t.whens {
        if let When::Near(id) = &w.when {
            if id != JAINA && kind(id).is_none() {
                return Err(bad(0, format!("when near {id}: nothing called that")));
            }
        }
        if let When::Step(n) = w.when {
            if n > t.steps.len() {
                return Err(bad(0, format!("when step {n}: the tale has {} steps", t.steps.len())));
            }
        }
        match &w.act {
            Do::Say(l) if !speaker(&l.who) => return Err(bad(0, format!("{} isn't an npc", l.who.clone().unwrap_or_default()))),
            Do::Show(id) | Do::Hide(id) if kind(id).is_none() => return Err(bad(0, format!("show/hide {id}: nothing called that"))),
            _ => {}
        }
    }
    Ok(())
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn n(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

fn line_text(l: &Line) -> String {
    match &l.who {
        Some(w) => format!("{w} {}", q(&l.text)),
        None => q(&l.text),
    }
}

/// The tale written out in its one canonical form (what a code carries; `parse(&write(t)) == t` up to rounding to
/// hundredths of a yard).
pub fn write(t: &Tale) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "tale {}", t.title);
    if let Some(b) = &t.by {
        let _ = writeln!(s, "by {b}");
    }
    if let Some(a) = &t.about {
        let _ = writeln!(s, "about {a}");
    }
    for x in &t.things {
        let face = if x.face != 0.0 { format!(" face {}", n(x.face)) } else { String::new() };
        let _ = match &x.kind {
            Kind::Npc { name, look } => writeln!(s, "npc {} {} look {look} at {} {}{face}", x.id, q(name), n(x.at[0]), n(x.at[1])),
            Kind::Item { name, prop } => writeln!(s, "item {} {} prop {prop} at {} {}{face}", x.id, q(name), n(x.at[0]), n(x.at[1])),
            Kind::Sign { text } => writeln!(s, "sign {} at {} {} {}", x.id, n(x.at[0]), n(x.at[1]), q(text)),
            Kind::Portal { to } => writeln!(s, "portal {} at {} {} to {} {}", x.id, n(x.at[0]), n(x.at[1]), n(to[0]), n(to[1])),
        };
    }
    for st in &t.steps {
        let _ = writeln!(s, "step {}", st.title);
        for l in &st.say {
            let _ = writeln!(s, "  say {}", line_text(l));
        }
        let _ = match &st.goal {
            Goal::Talk(id) => writeln!(s, "  goal talk {id}"),
            Goal::Collect(id) => writeln!(s, "  goal collect {id}"),
            Goal::Visit(id) => writeln!(s, "  goal visit {id}"),
            Goal::Defeat(k) => writeln!(s, "  goal defeat {k} dummies"),
            Goal::Reach { at, r } => writeln!(s, "  goal reach {} {} {}", n(at[0]), n(at[1]), n(*r)),
            Goal::Wait(k) => writeln!(s, "  goal wait {k}"),
        };
        if let Some(h) = &st.hint {
            let _ = writeln!(s, "  hint {h}");
        }
        for l in &st.done {
            let _ = writeln!(s, "  done {}", line_text(l));
        }
    }
    for w in &t.whens {
        let head = match &w.when {
            When::Enter { at, r } => format!("enter {} {} {}", n(at[0]), n(at[1]), n(*r)),
            When::Near(id) => format!("near {id}"),
            When::Timer(k) => format!("timer {k}"),
            When::Step(k) => format!("step {k}"),
        };
        let act = match &w.act {
            Do::Say(l) => format!("say {}", line_text(l)),
            Do::Teleport(at) => format!("teleport {} {}", n(at[0]), n(at[1])),
            Do::Show(id) => format!("show {id}"),
            Do::Hide(id) => format!("hide {id}"),
        };
        let _ = writeln!(s, "when {head} do {act}{}", if w.again { " again" } else { "" });
    }
    for r in &t.rewards {
        let _ = match r {
            Reward::Title(x) => writeln!(s, "reward title {x}"),
            Reward::Badge(x) => writeln!(s, "reward badge {x}"),
            Reward::Cheer => writeln!(s, "reward cheer"),
            Reward::Glow => writeln!(s, "reward glow"),
        };
    }
    s
}

/// One line per step, for a preview ("1. Find the lantern (collect the Lost Lantern)").
pub fn outline(t: &Tale) -> Vec<String> {
    t.steps.iter().enumerate().map(|(i, s)| format!("{}. {} ({})", i + 1, s.title, goal_words(t, &s.goal))).collect()
}

fn name_of(t: &Tale, id: &str) -> String {
    if id == JAINA {
        return "Jaina".into();
    }
    match t.things.iter().find(|x| x.id == id).map(|x| &x.kind) {
        Some(Kind::Npc { name, .. }) | Some(Kind::Item { name, .. }) => name.clone(),
        Some(Kind::Sign { .. }) => "the sign".into(),
        Some(Kind::Portal { .. }) => "the portal".into(),
        None => id.into(),
    }
}

/// What a goal asks, in words (the tracker's line).
pub fn goal_words(t: &Tale, g: &Goal) -> String {
    match g {
        Goal::Talk(id) => format!("talk to {}", name_of(t, id)),
        Goal::Collect(id) => format!("find {}", name_of(t, id)),
        Goal::Visit(id) => format!("go to {}", name_of(t, id)),
        Goal::Defeat(k) => format!("defeat {k} training dumm{}", if *k == 1 { "y" } else { "ies" }),
        Goal::Reach { at, .. } => format!("reach the spot at {}, {}", n(at[0]), n(at[1])),
        Goal::Wait(k) => format!("wait {k} s"),
    }
}

// ------------------------------------------------------------------------------------------------------------------
// playing a tale
// ------------------------------------------------------------------------------------------------------------------

/// What happened (all places relative to the Den's middle, yards).
#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    /// The player is here now (each move; and Jaina is at `jaina` when she is out).
    At { me: [f32; 2], jaina: Option<[f32; 2]> },
    /// A training dummy went down (refilled).
    Defeated,
    /// Time now (ms).
    Tick(u64),
    /// `.den tale talk` / a click: as if the player walked up to `id`.
    Use(String),
}

/// What to show.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// A line said: by an NPC (its id), Jaina, or the narrator (None).
    Say(Line),
    /// The tracker: step n of m, its title and the goal in words (and progress like "1/3").
    Track { n: usize, of: usize, title: String, goal: String },
    Hint(String),
    Show(String),
    Hide(String),
    Teleport([f32; 2]),
    Reward(Reward),
    /// The whole tale is done.
    Finished(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub tale: Tale,
    /// The step under way (0-based); == steps.len(): finished.
    pub step: usize,
    pub progress: u32,
    pub start_ms: u64,
    pub step_ms: u64,
    pub now_ms: u64,
    fired: Vec<bool>,
    inside: Vec<bool>,
    /// Items picked up (they stay away).
    pub taken: Vec<String>,
    pub hidden: Vec<String>,
    started: bool,
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl Run {
    pub fn new(tale: Tale, now_ms: u64) -> Run {
        let k = tale.whens.len();
        Run { tale, step: 0, progress: 0, start_ms: now_ms, step_ms: now_ms, now_ms, fired: vec![false; k], inside: vec![false; k], taken: Vec::new(), hidden: Vec::new(), started: false }
    }

    pub fn finished(&self) -> bool {
        self.step >= self.tale.steps.len()
    }

    /// Where a thing is now, if it is shown (an item picked up, or hidden, isn't).
    pub fn shown(&self, id: &str) -> bool {
        !self.taken.iter().any(|t| t == id) && !self.hidden.iter().any(|t| t == id)
    }

    /// The first acts: the tale's title, step 1, its lines. Call once, right after [`Run::new`].
    pub fn begin(&mut self) -> Vec<Act> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let mut out = vec![Act::Say(Line { who: None, text: format!("A tale begins: {}", self.tale.title) })];
        out.extend(self.enter_step());
        out
    }

    fn track(&self) -> Option<Act> {
        let s = self.tale.steps.get(self.step)?;
        let mut goal = goal_words(&self.tale, &s.goal);
        if let Goal::Defeat(k) = s.goal {
            goal = format!("{goal} ({}/{k})", self.progress);
        }
        Some(Act::Track { n: self.step + 1, of: self.tale.steps.len(), title: s.title.clone(), goal })
    }

    fn enter_step(&mut self) -> Vec<Act> {
        let mut out = Vec::new();
        self.progress = 0;
        self.step_ms = self.now_ms;
        let Some(s) = self.tale.steps.get(self.step).cloned() else { return out };
        out.extend(s.say.iter().cloned().map(Act::Say));
        out.extend(self.track());
        if let Some(h) = s.hint {
            out.push(Act::Hint(h));
        }
        let n = self.step + 1;
        for i in 0..self.tale.whens.len() {
            if self.tale.whens[i].when == When::Step(n) && !self.fired[i] {
                self.fired[i] = true;
                out.extend(self.act(i));
            }
        }
        out
    }

    fn act(&mut self, i: usize) -> Vec<Act> {
        match self.tale.whens[i].act.clone() {
            Do::Say(l) => vec![Act::Say(l)],
            Do::Teleport(at) => vec![Act::Teleport(at)],
            Do::Show(id) => {
                self.hidden.retain(|h| *h != id);
                vec![Act::Show(id)]
            }
            Do::Hide(id) => {
                if !self.hidden.contains(&id) {
                    self.hidden.push(id.clone());
                }
                vec![Act::Hide(id)]
            }
        }
    }

    fn complete_step(&mut self) -> Vec<Act> {
        let mut out = Vec::new();
        if let Some(s) = self.tale.steps.get(self.step) {
            out.extend(s.done.iter().cloned().map(Act::Say));
        }
        self.step += 1;
        if self.finished() {
            out.extend(self.tale.rewards.iter().cloned().map(Act::Reward));
            out.push(Act::Finished(self.tale.title.clone()));
        } else {
            out.extend(self.enter_step());
        }
        out
    }

    fn at_of(&self, id: &str, jaina: Option<[f32; 2]>) -> Option<[f32; 2]> {
        if id == JAINA {
            return jaina;
        }
        self.tale.things.iter().find(|x| x.id == id && self.shown(&x.id)).map(|x| x.at)
    }

    /// What happened, and what to show for it. Nothing after the tale is finished.
    pub fn on(&mut self, ev: &Ev) -> Vec<Act> {
        let mut out = Vec::new();
        if !self.started || self.finished() {
            return out;
        }
        match ev {
            Ev::Tick(now) => {
                self.now_ms = (*now).max(self.now_ms);
                for i in 0..self.tale.whens.len() {
                    if let When::Timer(s) = self.tale.whens[i].when {
                        if !self.fired[i] && self.now_ms.saturating_sub(self.start_ms) >= s as u64 * 1000 {
                            self.fired[i] = true;
                            out.extend(self.act(i));
                        }
                    }
                }
                if let Some(Goal::Wait(s)) = self.tale.steps.get(self.step).map(|s| s.goal.clone()) {
                    if self.now_ms.saturating_sub(self.step_ms) >= s as u64 * 1000 {
                        out.extend(self.complete_step());
                    }
                }
            }
            Ev::Defeated => {
                if let Some(Goal::Defeat(k)) = self.tale.steps.get(self.step).map(|s| s.goal.clone()) {
                    self.progress += 1;
                    if self.progress >= k {
                        out.extend(self.complete_step());
                    } else {
                        out.extend(self.track());
                    }
                }
            }
            Ev::Use(id) => {
                out.extend(self.reached(id));
            }
            Ev::At { me, jaina } => {
                // the triggers first (an `enter` beside a goal says its line before the step moves on)
                for i in 0..self.tale.whens.len() {
                    let spot = match &self.tale.whens[i].when {
                        When::Enter { at, r } => Some((*at, *r)),
                        When::Near(id) => self.at_of(id, *jaina).map(|a| (a, READ_YARDS)),
                        _ => None,
                    };
                    let Some((at, r)) = spot else { continue };
                    let inside = dist(*me, at) <= r;
                    if inside && !self.inside[i] && (!self.fired[i] || self.tale.whens[i].again) {
                        self.fired[i] = true;
                        out.extend(self.act(i));
                    }
                    self.inside[i] = inside;
                }
                // portals
                for x in self.tale.things.clone() {
                    if let Kind::Portal { to } = x.kind {
                        if self.shown(&x.id) && dist(*me, x.at) <= PORTAL_YARDS {
                            out.push(Act::Teleport(to));
                        }
                    }
                }
                // signs read when walked up to (once each while near: the glue shows them; the tale says it here)
                let goal = self.tale.steps.get(self.step).map(|s| s.goal.clone());
                let near = |id: &str, yards: f32, run: &Run| run.at_of(id, *jaina).is_some_and(|a| dist(*me, a) <= yards);
                let hit = match &goal {
                    Some(Goal::Talk(id)) => near(id, TALK_YARDS, self).then(|| id.clone()),
                    Some(Goal::Collect(id)) => near(id, PICK_YARDS, self).then(|| id.clone()),
                    Some(Goal::Visit(id)) => near(id, READ_YARDS, self).then(|| id.clone()),
                    Some(Goal::Reach { at, r }) => (dist(*me, *at) <= *r).then(String::new),
                    _ => None,
                };
                if let Some(id) = hit {
                    if id.is_empty() {
                        out.extend(self.complete_step());
                    } else {
                        out.extend(self.reached(&id));
                    }
                }
            }
        }
        out
    }

    /// The player is at `id` (walked up, or `.den tale use`): the step's goal, if it is that.
    fn reached(&mut self, id: &str) -> Vec<Act> {
        let mut out = Vec::new();
        let goal = self.tale.steps.get(self.step).map(|s| s.goal.clone());
        match goal {
            Some(Goal::Collect(g)) if g == id => {
                if !self.taken.contains(&g) {
                    self.taken.push(g.clone());
                }
                out.push(Act::Hide(g.clone()));
                out.push(Act::Say(Line { who: None, text: format!("You picked up {}.", name_of(&self.tale, &g)) }));
                out.extend(self.complete_step());
            }
            Some(Goal::Talk(g)) | Some(Goal::Visit(g)) if g == id => out.extend(self.complete_step()),
            _ => {}
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Jaina's own drafts (no assistant connected: the built-in utility brain and templates)
// ------------------------------------------------------------------------------------------------------------------

/// A word from the player's request as a prop word the catalog may have (the glue checks `.den find`, else a crate).
fn noun_after(ws: &[String], i: usize) -> Option<String> {
    const SKIP: &[&str] = &["the", "a", "an", "my", "our", "your", "some", "lost", "missing", "old", "little", "magic", "her", "his", "their", "of", "for", "and", "then", "to", "me"];
    ws.iter().skip(i).take(5).find(|w| !SKIP.contains(&w.as_str()) && !branded(w) && w.chars().all(|c| c.is_ascii_alphabetic()) && w.len() <= 16).cloned()
}

fn number_near(ws: &[String], i: usize) -> Option<u32> {
    const NUM: &[(&str, u32)] = &[("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6), ("ten", 10), ("a", 1)];
    ws.iter().skip(i).take(3).find_map(|w| w.parse::<u32>().ok().or_else(|| NUM.iter().find(|n| n.0 == w).map(|n| n.1))).map(|n| n.clamp(1, 20))
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Jaina's own draft of a tale from the player's words ("make a quest where you find my lost lantern, then beat 3
/// dummies and reward the title Lantern Finder"): goals in the order they are asked for, Jaina as the one who gives
/// it, places round the Den's middle within `room` yards. Always a tale that [`parse`] takes.
pub fn draft(request: &str, room: f32) -> String {
    let low: String = request.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect();
    let ws: Vec<String> = low.split_whitespace().map(String::from).collect();
    let r = room.clamp(8.0, 60.0) * 0.6;
    let mut items: Vec<(String, String)> = Vec::new();
    let mut steps: Vec<String> = Vec::new();
    let mut things: Vec<String> = Vec::new();
    let mut whens: Vec<String> = Vec::new();
    let mut title: Option<String> = None;
    let mut reward: Option<String> = None;
    let mut corner = 0usize;
    let spot = |k: usize| -> (f32, f32) {
        let a = k as f32 * 2.3 + 0.6;
        ((a.cos() * r * 10.0).round() / 10.0, (a.sin() * r * 10.0).round() / 10.0)
    };
    let mut i = 0;
    while i < ws.len() {
        let w = ws[i].as_str();
        match w {
            "find" | "collect" | "gather" | "fetch" | "bring" | "retrieve" | "recover" | "rescue" | "save" if steps.len() < 6 => {
                if let Some(noun) = noun_after(&ws, i + 1).filter(|n| !["dummy", "dummies", "quest", "tale"].contains(&n.as_str())) {
                    let id: String = noun.chars().take(12).collect();
                    let id = if items.iter().any(|x| x.0 == id) || things.iter().any(|t| t.contains(&format!(" {id} "))) { format!("{id}{}", items.len() + 1) } else { id };
                    let (x, y) = spot(corner);
                    corner += 1;
                    let name = title_case(&format!("lost {noun}"));
                    things.push(format!("item {id} \"{name}\" prop {noun} at {x} {y}"));
                    items.push((id.clone(), noun.clone()));
                    steps.push(format!("step Find the {noun}\n  goal collect {id}\n  hint Look around the edge of the Den."));
                    title.get_or_insert_with(|| format!("The {}", title_case(&format!("lost {noun}"))));
                }
            }
            "beat" | "defeat" | "fight" | "kill" | "spar" | "train" | "battle" if steps.len() < 6 => {
                let k = number_near(&ws, i + 1).unwrap_or(3);
                steps.push(format!("step Prove your strength\n  say jaina \"Show me what you can do on the training dummies.\"\n  goal defeat {k} dummies"));
                title.get_or_insert_with(|| "The Trial of Strength".into());
            }
            "reach" | "visit" | "climb" | "explore" | "go" | "travel" | "walk" if steps.len() < 6 => {
                let (x, y) = spot(corner);
                corner += 1;
                let place = noun_after(&ws, i + 1).filter(|n| !["to", "there"].contains(&n.as_str())).unwrap_or_else(|| "lookout".into());
                steps.push(format!("step Go to the {place}\n  goal reach {x} {y} 4\n  hint It's toward the edge."));
                whens.push(format!("when enter {x} {y} 4 do say \"You can see the whole Den from the {place}.\""));
                title.get_or_insert_with(|| format!("The Way to the {}", title_case(&place)));
            }
            "title" | "called" | "named" if ws.get(i.wrapping_sub(1)).is_some_and(|p| p == "the" || p == "a" || p == "reward" || p == "title") || w != "title" => {
                let rest: Vec<&str> = ws.iter().skip(i + 1).take(3).map(String::as_str).take_while(|x| !["and", "then", "with", "where", "that"].contains(x)).collect();
                if !rest.is_empty() {
                    let t = title_case(&rest.join(" "));
                    if w == "title" || ws.get(i.wrapping_sub(1)).is_some_and(|p| p == "reward") {
                        reward = Some(t);
                    } else {
                        title = Some(t);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    if steps.is_empty() {
        // nothing recognised: a short tour of the Den that ends back at Jaina
        let (x, y) = spot(0);
        steps.push(format!("step See the Den\n  say jaina \"Let me show you around. Walk out to the far side.\"\n  goal reach {x} {y} 5"));
        title.get_or_insert_with(|| "A Walk Round the Den".into());
    }
    // always: Jaina gives it, and it ends back with her
    let mut s = String::new();
    let title = clean_text(&title.unwrap_or_else(|| "Jaina's Tale".into()), MAX_TITLE).unwrap_or_else(|_| "Jaina's Tale".into());
    let _ = writeln!(s, "tale {title}");
    let _ = writeln!(s, "by Jaina");
    for t in &things {
        let _ = writeln!(s, "{t}");
    }
    let _ = writeln!(s, "step Hear Jaina out\n  say jaina \"I have a little tale for you. Come and talk to me.\"\n  goal talk jaina");
    for st in steps.iter().take(MAX_STEPS - 2) {
        let _ = writeln!(s, "{st}");
    }
    let _ = writeln!(s, "step Back to Jaina\n  goal talk jaina\n  done jaina \"Well done! That's the tale.\"");
    for w in whens.iter().take(MAX_WHENS) {
        let _ = writeln!(s, "{w}");
    }
    match reward.and_then(|r| clean_text(&r, 32).ok()) {
        Some(r) => {
            let _ = writeln!(s, "reward title {r}");
        }
        None => {
            let _ = writeln!(s, "reward cheer");
        }
    }
    // a last guard: if anything above made a line parse refuses, the plain tour (never a broken draft)
    match parse(&s) {
        Ok(t) => write(&t),
        Err(_) => write(&parse("tale Jaina's Tale\nby Jaina\nstep Hear Jaina out\n  goal talk jaina\nreward cheer\n").expect("the plain tale")),
    }
}

/// What the player's own assistant is told a tale is (the Agents page's tool, and the ask): the whole format in a few
/// lines.
pub const SPEC: &str = "A Den tale is plain text, one statement a line: `tale <title>` first; `by <name>`; `npc <id> \"<Name>\" look <villager|guard|mage|dwarf|elf|ghost|dummy> at <x> <y>`; `item <id> \"<Name>\" prop <catalog word> at <x> <y>`; `sign <id> at <x> <y> \"<text>\"`; `portal <id> at <x> <y> to <x> <y>`; then steps: `step <title>` with `goal talk <npc|jaina>` | `goal collect <item>` | `goal visit <id>` | `goal defeat <n> dummies` | `goal reach <x> <y> [r]` | `goal wait <s>`, and `say [npc] \"...\"`, `hint ...`, `done [npc] \"...\"`; `when enter <x> <y> <r> | near <id> | timer <s> | step <n> do say|teleport|show|hide ... [again]`; `reward title <text> | badge <text> | cheer | glow`. Places are yards from the Den's middle (within 200). Ids: a-z 0-9 - _. At most 8 KB, 24 steps. No links. Rewards are looks only.";

#[cfg(test)]
mod tests {
    use super::*;

    const LANTERN: &str = r#"
tale The Lost Lantern
by Wren
about A small tale on the island.
npc mira "Mira" look villager at 4 2
item lantern "Mira's Lantern" prop lantern at 12 -6
sign welcome at 0 3 "Welcome! # not a comment inside quotes"
portal up at -10 0 to 20 20
step Find Mira's lantern    # a comment
  say mira "My lantern rolled off. Would you find it?"
  goal collect lantern
  hint It glows near the rocks.
step Bring it back
  goal talk mira
  done mira "You found it! Thank you."
step Train a little
  goal defeat 2 dummies
when enter 0 0 6 do say "The old oak hums."
when near welcome do say mira "Read it twice." again
when timer 30 do say "A breeze."
when step 3 do show lantern
reward title Lantern Finder
reward cheer
"#;

    #[test]
    fn a_whole_tale_reads_and_writes_back_the_same() {
        let t = parse(LANTERN).unwrap();
        assert_eq!(t.title, "The Lost Lantern");
        assert_eq!(t.by.as_deref(), Some("Wren"));
        assert_eq!(t.things.len(), 4);
        assert_eq!(t.steps.len(), 3);
        assert!(matches!(&t.things[2].kind, Kind::Sign { text } if text.contains("# not a comment")));
        assert_eq!(t.steps[0].goal, Goal::Collect("lantern".into()));
        assert_eq!(t.steps[2].goal, Goal::Defeat(2));
        assert_eq!(t.whens.len(), 4);
        assert!(t.whens[1].again);
        let w = write(&t);
        assert_eq!(parse(&w).unwrap(), t, "canonical form round trip:\n{w}");
        assert_eq!(write(&parse(&w).unwrap()), w, "stable");
        assert_eq!(outline(&t)[0], "1. Find Mira's lantern (find Mira's Lantern)");
    }

    #[test]
    fn problems_name_their_line_in_plain_words() {
        let cases: &[(&str, &str)] = &[
            ("step x\n", "starts with"),
            ("tale T\n", "at least one step"),
            ("tale T\nstep A\n", "no goal"),
            ("tale T\nstep A\n  goal talk bob\n", "no such npc"),
            ("tale T\nstep A\n  goal collect bob\n", "no such item"),
            ("tale T\nnpc a \"A\" look robot at 0 0\nstep A\n goal talk a\n", "look:"),
            ("tale T\nnpc a \"A\" look mage at 0 999\nstep A\n goal talk a\n", "place"),
            ("tale T\nnpc jaina \"J\" look mage at 0 0\nstep A\n goal talk jaina\n", "always here"),
            ("tale T\nstep A\n goal talk jaina\n say \"see https://x.y\"\n", "no links"),
            ("tale T\nstep A\n goal talk jaina\n say \"my wow account\"\n", "can't be shown"),
            ("tale T\nstep A\n goal defeat 99 dummies\n", "1 to 50"),
            ("tale T\nstep A\n goal talk jaina\n goal talk jaina\n", "one goal"),
            ("tale T\nstep A\n goal talk jaina\nwhen timer 2 do say \"x\"\n", "5-3600"),
            ("tale T\nstep A\n goal talk jaina\nwhen step 4 do say \"x\"\n", "has 1 steps"),
            ("tale T\nstep A\n goal talk jaina\nwhen timer 9 do say \"x\" again\n", "only for enter"),
            ("tale T\nstep A\n goal talk jaina\nreward gold 100\n", "looks only"),
            ("tale T\nstep A\n goal talk jaina\nlua os.execute()\n", "unknown word"),
            ("tale T\nstep A\n goal talk jaina\n say \"open\n", "never closed"),
            ("tale T\ngoal talk jaina\n", "belongs to a step"),
        ];
        for (text, want) in cases {
            let e = parse(text).expect_err(text);
            assert!(e.to_string().contains(want), "{text:?}: {e}");
        }
        let big = format!("tale T\nstep A\n goal talk jaina\n{}", "# pad\n".repeat(2000));
        assert!(parse(&big).unwrap_err().what.contains("KB"));
    }

    fn me(x: f32, y: f32) -> Ev {
        Ev::At { me: [x, y], jaina: Some([0.0, 0.0]) }
    }

    #[test]
    fn playing_the_lantern_tale_from_start_to_reward() {
        let mut r = Run::new(parse(LANTERN).unwrap(), 1000);
        let a = r.begin();
        assert!(matches!(&a[0], Act::Say(l) if l.text.contains("The Lost Lantern")));
        assert!(a.iter().any(|x| matches!(x, Act::Track { n: 1, of: 3, .. })));
        assert!(a.iter().any(|x| matches!(x, Act::Hint(h) if h.contains("rocks"))));
        assert!(r.begin().is_empty(), "begin once");
        // the oak's trigger once, not again on coming back
        let a = r.on(&me(-3.0, -2.0));
        assert!(a.iter().any(|x| matches!(x, Act::Say(l) if l.text.contains("oak"))));
        r.on(&me(30.0, 30.0));
        assert!(!r.on(&me(-3.0, -2.0)).iter().any(|x| matches!(x, Act::Say(l) if l.text.contains("oak"))));
        // the sign's again-trigger fires each time you come back
        assert!(r.on(&me(0.0, 3.0)).iter().any(|x| matches!(x, Act::Say(l) if l.text.contains("twice"))));
        r.on(&me(30.0, 30.0));
        assert!(r.on(&me(0.0, 3.5)).iter().any(|x| matches!(x, Act::Say(l) if l.text.contains("twice"))));
        // talking to Mira too early does nothing; picking the lantern up moves on
        assert!(r.on(&me(4.0, 2.0)).iter().all(|x| !matches!(x, Act::Track { n: 2, .. })));
        let a = r.on(&me(11.0, -5.0));
        assert!(a.contains(&Act::Hide("lantern".into())), "{a:?}");
        assert_eq!(r.step, 1);
        assert!(!r.shown("lantern"));
        let a = r.on(&me(4.0, 2.5));
        assert!(a.iter().any(|x| matches!(x, Act::Say(l) if l.who.as_deref() == Some("mira") && l.text.contains("Thank"))));
        assert!(a.contains(&Act::Show("lantern".into())), "when step 3 do show");
        assert_eq!(r.step, 2);
        // the portal
        assert!(r.on(&me(-10.0, 0.5)).contains(&Act::Teleport([20.0, 20.0])));
        // two dummies
        let a = r.on(&Ev::Defeated);
        assert!(a.iter().any(|x| matches!(x, Act::Track { goal, .. } if goal.ends_with("(1/2)"))));
        let a = r.on(&Ev::Defeated);
        assert!(a.contains(&Act::Reward(Reward::Title("Lantern Finder".into()))));
        assert!(a.contains(&Act::Finished("The Lost Lantern".into())));
        assert!(r.finished());
        assert!(r.on(&Ev::Defeated).is_empty() && r.on(&Ev::Tick(99_999)).is_empty(), "nothing after the end");
    }

    #[test]
    fn timers_waits_and_use() {
        let t = parse("tale T\nstep Wait\n goal wait 5\nstep Talk\n goal talk jaina\nwhen timer 10 do say \"ten\"\n").unwrap();
        let mut r = Run::new(t, 0);
        r.begin();
        assert!(r.on(&Ev::Tick(4_000)).is_empty());
        assert!(r.on(&Ev::Tick(5_000)).iter().any(|x| matches!(x, Act::Track { n: 2, .. })));
        assert!(r.on(&Ev::Tick(10_000)).iter().any(|x| matches!(x, Act::Say(l) if l.text == "ten")));
        assert!(r.on(&Ev::Tick(20_000)).is_empty(), "a timer fires once");
        // jaina far away: no talk; `.den tale use jaina` counts
        assert!(r.on(&Ev::At { me: [50.0, 0.0], jaina: Some([0.0, 0.0]) }).is_empty());
        assert!(r.on(&Ev::At { me: [0.0, 0.0], jaina: None }).is_empty(), "jaina not out: nobody to talk to");
        assert!(r.on(&Ev::Use("jaina".into())).iter().any(|x| matches!(x, Act::Finished(_))));
    }

    #[test]
    fn jaina_drafts_parse_for_any_words() {
        let asks = [
            "make a quest where you find my lost lantern then beat 3 dummies and reward the title Lantern Finder",
            "a quest to rescue the cat and go to the lookout",
            "talk to me, then explore the cliffs, then defeat five dummies",
            "something fun",
            "",
            "find find find find find find find find",
            "find the wow box", // a brand word in a noun never reaches the text
            "beat 9999 dummies",
            "collect \"quotes\" and \\ backslashes # hash",
        ];
        for a in asks {
            let d = draft(a, 20.0);
            let t = parse(&d).unwrap_or_else(|e| panic!("{a}: {e}\n{d}"));
            assert!(t.steps.len() >= 2 && t.steps.len() <= MAX_STEPS, "{a}");
            assert!(!branded(&d), "{a}: {d}");
            assert_eq!(t.steps[0].goal, Goal::Talk(JAINA.into()), "Jaina gives it");
        }
        let t = parse(&draft(asks[0], 20.0)).unwrap();
        assert!(t.steps.iter().any(|s| matches!(&s.goal, Goal::Collect(id) if id == "lantern")));
        assert!(t.steps.iter().any(|s| s.goal == Goal::Defeat(3)));
        assert!(t.rewards.contains(&Reward::Title("Lantern Finder".into())), "{:?}", t.rewards);
        assert!(parse(&draft("beat 9999 dummies", 20.0)).unwrap().steps.iter().any(|s| s.goal == Goal::Defeat(20)));
    }

    /// A tiny deterministic fuzz: random edits of a good tale never panic; whatever parses writes back the same.
    #[test]
    fn fuzzed_tales_never_panic_and_round_trip() {
        let mut seed: u64 = 0x5EED_7A1E;
        let mut rnd = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n.max(1) as u64) as usize
        };
        let pieces = ["\"", "#", " ", "\n", "step X\n", "goal talk jaina\n", "999", "-1", "at", "do", "again", "é", "\\\"", "\t", "when enter 1 1 2 do say \"x\"\n", "npc b \"B\" look elf at 1 1\n"];
        for _ in 0..3000 {
            let mut s: Vec<char> = LANTERN.chars().collect();
            for _ in 0..1 + rnd(4) {
                let at = rnd(s.len());
                match rnd(3) {
                    0 => {
                        let len = rnd(12).min(s.len() - at);
                        s.drain(at..at + len);
                    }
                    _ => {
                        let p = pieces[rnd(pieces.len())];
                        for (k, c) in p.chars().enumerate() {
                            s.insert((at + k).min(s.len()), c);
                        }
                    }
                }
            }
            let text: String = s.into_iter().collect();
            if let Ok(t) = parse(&text) {
                let w = write(&t);
                assert_eq!(write(&parse(&w).unwrap_or_else(|e| panic!("{e}\n{w}"))), w);
                let mut r = Run::new(t, 0);
                r.begin();
                for k in 0..20 {
                    r.on(&me(rnd(60) as f32 - 30.0, rnd(60) as f32 - 30.0));
                    r.on(&Ev::Defeated);
                    r.on(&Ev::Tick(k * 4000));
                }
            }
        }
    }
}
