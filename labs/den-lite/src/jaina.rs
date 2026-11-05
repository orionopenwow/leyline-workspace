//! Jaina's brain in Den Lite (owner 4 Nov: "upgrade the way Jaina thinks, acts, and what she spawns"). Pure logic:
//! no packets, no files, no clock, no locks, so all of it is tested here; world.rs feeds it what happens and does
//! what it decides (companion.rs keeps her body: where she stands, her walks, the assistant's files).
//!
//! * **Thinking.** A small utility AI: four needs that grow with time (company, curiosity, helping, rest), a mood
//!   from them and from what happened (calm, cheerful, excited, curious, sleepy), and once a second she scores what
//!   she could do (react to the newest thing you did, look around the Den, a gesture while you stand still, suggest a
//!   small set) and does the best one, or nothing. Randomness is seeded ([`Rng`]), so a test sees the same choices
//!   every time.
//! * **Memory of the visit.** What you placed (her guess at your favourite piece), deleted, saved, the friends who
//!   came, the sets you kept or sent away (a set sent away twice is not suggested again; two unasked suggestions
//!   sent away and she stops suggesting by herself), and the pieces she put down ([`Brain::mine`]).
//! * **Timing, so she doesn't spam** (owner 1 Nov 23:46/47: "not spam the chat"; she speaks when spoken to). What
//!   she says unasked is a toast in the Den bar (never the chat), at most one every [`LINE_GAP_NORMAL_MS`] (Normal)
//!   and [`LINES_CAP_NORMAL`] a visit, none in her first [`WARMUP_MS`]; gestures at most one every [`EMOTE_GAP_MS`].
//!   Quiet ("Jaina, quiet", the Den panel): no unasked words and no suggestions at all, gestures only. Chatty: more.
//! * **Spawning with a purpose.** "Jaina, help" (or now and then by herself, while you build): a small themed set
//!   ([`THEMES`]: in season first, one with your favourite piece next) shown as a sparkling preview copy in front of
//!   you; "Jaina, yes" keeps it (a stamp: one undo), "no" sends it away, a minute unanswered and it goes. Critters
//!   ([`CRITTERS`], at most [`MAX_CRITTERS`], never saved). "Jaina, tidy up" asks first, then takes away only what
//!   she put down and you haven't moved ([`tidy_targets`]); she never touches your own pieces. At most [`MAX_MINE`]
//!   pieces of hers in a Den.
//! * **Words** ([`word`]): quiet, talk, chatty, help, yes, no, tidy up, a pet, pets away, what did I build, build a
//!   <theme>. The old ones (follow, stay, come, tips, go away) stay in companion.rs `command`.
//!
//! Lines: `jaina-lines.txt` beside this file (TODO(owner): his voice), read in at build time.

use crate::companion::At;
use std::collections::BTreeMap;

// ---------------------------------------------------------------- timing (milliseconds)

/// She thinks once a second.
pub const THINK_MS: u64 = 1000;
/// No unasked words in a visit's first minute and a half (the greeting and the Den bar's intro come first).
pub const WARMUP_MS: u64 = 90_000;
/// Between unasked lines: Normal, Chatty.
pub const LINE_GAP_NORMAL_MS: u64 = 180_000;
pub const LINE_GAP_CHATTY_MS: u64 = 60_000;
/// Unasked lines in one visit, at most.
pub const LINES_CAP_NORMAL: u32 = 8;
pub const LINES_CAP_CHATTY: u32 = 30;
/// Between gestures (emotes).
pub const EMOTE_GAP_MS: u64 = 6_000;
/// A thing you did is reacted to within this, or not at all.
pub const REACT_FRESH_MS: u64 = 5_000;
/// You count as standing still after this long without moving.
pub const STILL_MS: u64 = 25_000;
/// She doesn't wander off in a visit's first minute.
pub const EXPLORE_AFTER_MS: u64 = 60_000;
/// How long a look around keeps her away from your side (unless you walk off).
pub const EXPLORE_MS: u64 = 15_000;
/// How far from you she looks around (companion.rs LEASH: further and she comes back).
pub const EXPLORE_RADIUS: f32 = 7.0;
/// Between idle gestures.
pub const IDLE_GAP_MS: u64 = 45_000;
/// A suggestion's preview stays this long unanswered.
pub const OFFER_MS: u64 = 60_000;
/// Unasked suggestions: the first not before this far into a visit, then this far apart; a visit's most.
pub const OFFER_FIRST_MS: u64 = 300_000;
pub const OFFER_GAP_MS: u64 = 600_000;
pub const OFFERS_CAP_NORMAL: u32 = 2;
pub const OFFERS_CAP_CHATTY: u32 = 4;
/// Pieces you placed lately (within OFFER_RECENT_MS) before she suggests by herself.
pub const OFFER_AFTER_PLACED: usize = 3;
pub const OFFER_RECENT_MS: u64 = 120_000;
/// "Tidy up?" waits this long for a yes.
pub const ASK_MS: u64 = 30_000;
/// Her pieces in one Den, at most.
pub const MAX_MINE: usize = 48;
/// Critters out at once, at most; how far they roam from where they came; how often they move.
pub const MAX_CRITTERS: usize = 3;
pub const CRITTER_ROAM: f32 = 3.0;
pub const CRITTER_STEP_MIN_MS: u64 = 3_000;
pub const CRITTER_STEP_MAX_MS: u64 = 8_000;
/// More than this many pieces at once is a stamp or a load, not one you placed.
pub const LOTS: usize = 8;

// ---------------------------------------------------------------- gestures

/// SMSG_EMOTE (3.3.5a): u32 the emote, u64 the unit's guid (AzerothCore Unit::HandleEmoteCommand).
pub const SMSG_EMOTE: u16 = 0x103;
/// One-shot emotes (AzerothCore SharedDefines.h `Emote`): played once, no state to clear afterwards.
pub const TALK: u32 = 1;
pub const BOW: u32 = 2;
pub const WAVE: u32 = 3;
pub const CHEER: u32 = 4;
pub const EXCLAMATION: u32 = 5;
pub const QUESTION: u32 = 6;
pub const LAUGH: u32 = 11;
pub const KNEEL: u32 = 16;
pub const APPLAUD: u32 = 21;
pub const SHY: u32 = 24;
pub const POINT: u32 = 25;
pub const SALUTE: u32 = 66;
pub const YES: u32 = 273;
pub const NO: u32 = 274;

/// SMSG_EMOTE's body.
pub fn emote_body(guid: u64, emote: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(12);
    b.extend_from_slice(&emote.to_le_bytes());
    b.extend_from_slice(&guid.to_le_bytes());
    b
}

// ---------------------------------------------------------------- seeded randomness

/// SplitMix64: small, fast, the same numbers for the same seed on every machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// 0 to n-1 (0 when n is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    /// 0.0 up to 1.0.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.unit() < p
    }
    pub fn between(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi.saturating_sub(lo) + 1)
    }
}

// ---------------------------------------------------------------- lines

pub const LINES_TEXT: &str = include_str!("jaina-lines.txt");

/// The lines file read: (key, line) in file order; `#` comments and blank lines skipped.
pub fn parse_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('|')?;
            let (k, v) = (k.trim(), v.trim());
            (!k.is_empty() && !v.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

/// `{name}` and the others filled in.
pub fn fill(line: &str, with: &[(&str, &str)]) -> String {
    let mut s = line.to_string();
    for (k, v) in with {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// The keys the code says (every one must be in the file: a test checks).
pub const KEYS: &[&str] = &[
    "place.first", "place", "place.lots", "place.again", "delete", "delete.lots", "fly", "land", "fight", "fight.won",
    "fight.lost", "guest", "guest.left", "save", "grow", "built", "idle.calm", "idle.cheerful", "idle.excited",
    "idle.curious", "idle.sleepy", "explore", "offer", "offer.season", "offer.accept", "offer.reject", "offer.expired",
    "offer.off", "offer.none", "offer.busy", "offer.limit", "offer.nospace", "theme.unknown", "tidy.ask", "tidy.done",
    "tidy.none", "tidy.cancel", "critter.new", "critter.full", "critter.gone", "critter.none", "talk.quiet",
    "talk.normal", "talk.chatty", "memory.none", "memory", "memory.nofav",
];

// ---------------------------------------------------------------- what happens, how she is

/// Something that happened in the Den (world.rs tells her).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// You placed `n` pieces (a stamp or a load: many); `entry`: the first one's.
    Placed { entry: u32, n: usize },
    /// Pieces went (a delete, an undo).
    Deleted { n: usize },
    Saved,
    Grew,
    FlyStart,
    FlyEnd,
    /// A fight in the Dojo, a duel on the full Den.
    FightStart,
    FightEnd { won: bool },
    GuestArrived,
    GuestLeft,
    /// She finished a build (dojo.rs).
    Built,
}

/// How much she says by herself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Talk {
    /// Nothing unasked, no suggestions; gestures, and answers when spoken to.
    Quiet,
    #[default]
    Normal,
    Chatty,
}

impl Talk {
    pub fn word(self) -> &'static str {
        match self {
            Talk::Quiet => "quiet",
            Talk::Normal => "normal",
            Talk::Chatty => "chatty",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Calm,
    Cheerful,
    Excited,
    Curious,
    Sleepy,
}

impl Mood {
    pub fn word(self) -> &'static str {
        match self {
            Mood::Calm => "calm",
            Mood::Cheerful => "cheerful",
            Mood::Excited => "excited",
            Mood::Curious => "curious",
            Mood::Sleepy => "sleepy",
        }
    }
    /// Her gesture for a moment of nothing in particular.
    pub fn gesture(self) -> u32 {
        match self {
            Mood::Calm => BOW,
            Mood::Cheerful => LAUGH,
            Mood::Excited => CHEER,
            Mood::Curious => POINT,
            Mood::Sleepy => KNEEL,
        }
    }
}

/// What she is doing, for the Den panel and the heartbeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Doing {
    #[default]
    Following,
    Waiting,
    Exploring,
    Offering,
    Building,
    Flying,
}

impl Doing {
    pub fn word(self) -> &'static str {
        match self {
            Doing::Following => "follow",
            Doing::Waiting => "stay",
            Doing::Exploring => "explore",
            Doing::Offering => "offer",
            Doing::Building => "build",
            Doing::Flying => "fly",
        }
    }
}

/// Needs, 0 (met) to 1 (pressing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Needs {
    pub company: f32,
    pub curiosity: f32,
    pub helping: f32,
    pub rest: f32,
}

impl Default for Needs {
    fn default() -> Self {
        Needs { company: 0.2, curiosity: 0.3, helping: 0.0, rest: 0.0 }
    }
}

/// What she remembers of this visit.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Memory {
    /// Pieces you placed, by entry (a stamp or a load counts once, not per piece).
    pub placed: BTreeMap<u32, u32>,
    pub pieces: u32,
    pub deleted: u32,
    pub saves: u32,
    pub guests: u32,
    pub flights: u32,
    pub fights: u32,
    pub builds: u32,
    /// Theme keys: kept, sent away (each time).
    pub kept: Vec<&'static str>,
    pub sent_away: Vec<&'static str>,
    /// Times you placed (for "while you build").
    pub recent: Vec<u64>,
}

impl Memory {
    /// Your favourite piece: the one placed most (at least twice; a tie: the lower entry).
    pub fn favourite(&self) -> Option<(u32, u32)> {
        self.placed.iter().filter(|(_, n)| **n >= 2).max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(e, n)| (*e, *n))
    }
    fn recent_since(&self, ms: u64) -> usize {
        self.recent.iter().filter(|t| **t >= ms).count()
    }
}

/// A question waiting for "yes" or "no".
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// A set's preview (its objects, world.rs makes them), in front of `at` facing `o`.
    Offer { theme: &'static str, at: At, guids: Vec<u64>, asked: bool, until: u64 },
    /// "Take my pieces away?"
    Tidy { guids: Vec<u64>, until: u64 },
}

/// One piece she put down: its guid and where it stood then (moved since: it is yours now).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mine {
    pub guid: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A critter she brought: its unit guid, creature entry, where it came, where it is now, its next step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Critter {
    pub guid: u64,
    pub entry: u32,
    pub home: At,
    pub at: At,
    pub next: u64,
}

/// What she decided (world.rs does it).
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// An unasked line: a toast in the Den bar (she turns to you first, when standing).
    Line(String),
    Emote(u32),
    /// A look around the Den: walk there (around the props) and linger.
    Wander(At),
    /// Suggest a set: world.rs shows the preview in front of you, then calls [`Brain::offered`].
    Offer(&'static str),
    /// The unanswered preview goes (these objects).
    Withdraw(Vec<u64>),
}

/// What she senses each tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sense {
    pub ms: u64,
    pub player: At,
    pub her: At,
    /// She is on her way somewhere.
    pub walking: bool,
    /// Out and told to follow (not "stay").
    pub following: bool,
    /// You are in the air.
    pub flying: bool,
    /// Her build is going (dojo.rs).
    pub building: bool,
    /// The Den's middle and how far from it is inside.
    pub mid: At,
    pub reach: f32,
    /// 1 to 12 (the PC's month: seasonal sets).
    pub month: u32,
}

// ---------------------------------------------------------------- themes and critters

/// A small set she suggests: its key (what you say), its name in her lines, other words for it, the months it is in
/// season (empty: any time), its pieces (entry, yards ahead, yards to the left, yaw in degrees), relative to the spot
/// in front of you. Every entry is in Den Lite's catalog (objects.rs; a test in world.rs checks).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub key: &'static str,
    pub title: &'static str,
    pub words: &'static [&'static str],
    pub months: &'static [u32],
    pub pieces: &'static [(u32, f32, f32, f32)],
}

pub const THEMES: &[Theme] = &[
    Theme { key: "corner", title: "cozy corner", words: &["corner", "seat", "seating", "chair"], months: &[],
        pieces: &[(181396, 0.6, 0.0, 0.0), (2413, 0.0, 0.0, 270.0), (182093, 1.2, 0.0, 0.0), (180765, 1.2, 0.0, 0.0)] },
    Theme { key: "lights", title: "ring of lights", words: &["lights", "light", "lamps", "lanterns", "torches"], months: &[],
        pieces: &[(180043, 0.0, -1.6, 0.0), (180043, 0.0, 1.6, 0.0), (179977, 1.6, -0.9, 0.0), (179977, 1.6, 0.9, 0.0)] },
    Theme { key: "flowers", title: "flower bed", words: &["flowers", "flower", "flowerbed", "spring", "plants"], months: &[3, 4, 5],
        pieces: &[(2109, 0.0, 0.0, 0.0), (2109, 0.9, 0.5, 40.0), (181103, 0.4, -0.7, 0.0), (181087, -0.6, 0.4, 0.0), (24388, 0.0, -2.0, 0.0)] },
    Theme { key: "picnic", title: "picnic", words: &["picnic", "summer", "lunch"], months: &[6, 7, 8],
        pieces: &[(188346, 0.0, 0.0, 0.0), (181305, 0.0, 0.0, 0.0), (2562, 0.0, 0.2, 0.0), (2687, 0.2, -0.2, 0.0), (24538, 0.0, 1.5, 0.0), (24538, 0.0, -1.5, 180.0)] },
    Theme { key: "stall", title: "market stall", words: &["stall", "stand", "shop", "crates", "barrels"], months: &[],
        pieces: &[(179967, 0.0, 0.0, 0.0), (179969, 0.9, 0.0, 15.0), (92527, 0.4, 0.8, 0.0), (183993, -0.6, 0.6, 0.0)] },
    Theme { key: "harvest", title: "harvest corner", words: &["harvest", "autumn", "fall", "hay"], months: &[9, 10, 11],
        pieces: &[(19429, 0.0, 0.0, 0.0), (195192, 1.0, 0.6, 0.0), (190559, 1.0, -0.6, 0.0), (179977, -0.8, 0.9, 0.0)] },
    Theme { key: "candles", title: "candlelit corner", words: &["candles", "candle", "candlelit", "spooky", "hallows"], months: &[10],
        pieces: &[(1558, 0.0, 0.0, 0.0), (1558, 0.5, 0.4, 0.0), (1558, 0.5, -0.4, 0.0), (190692, 0.9, 0.0, 0.0), (180043, -0.8, 0.0, 0.0)] },
    Theme { key: "winter", title: "winter tree", words: &["winter", "tree", "snow", "snowy", "festive"], months: &[12, 1, 2],
        pieces: &[(178557, 0.0, 0.0, 0.0), (180765, 1.2, 0.8, 0.0), (180765, 1.2, -0.8, 0.0), (188701, -1.2, 0.0, 0.0)] },
];

/// The theme for a word ("a winter tree": "winter"), or None.
pub fn theme(word: &str) -> Option<&'static Theme> {
    let w = word.trim().to_lowercase();
    let w = w.trim_end_matches("'s");
    THEMES.iter().find(|t| t.key == w || t.words.contains(&w))
}

pub fn theme_by_key(key: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.key == key)
}

pub fn in_season(t: &Theme, month: u32) -> bool {
    t.months.contains(&month)
}

/// A critter she can bring: creature entry (AzerothCore world database; its model from creatures.rs), its name,
/// the words for it.
pub const CRITTERS: &[(u32, &str, &[&str])] = &[
    (721, "rabbit", &["rabbit", "bunny", "hare"]),
    (1412, "squirrel", &["squirrel"]),
    (2620, "prairie dog", &["prairie", "gopher"]),
    (620, "chicken", &["chicken", "hen"]),
    (890, "fawn", &["fawn", "deer"]),
    (1933, "sheep", &["sheep", "lamb"]),
];

pub fn critter_name(entry: u32) -> Option<&'static str> {
    CRITTERS.iter().find(|c| c.0 == entry).map(|c| c.1)
}

/// A critter's guid: HighGuid::Unit (0xF130), its entry, a counter from 0xA000 (her own is 1, dummies elsewhere).
pub fn critter_guid(entry: u32, n: u32) -> u64 {
    (0xF130u64 << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (0xA000 + (n as u64 & 0xFFF))
}

/// The pieces of a set laid out in front of `at` (its spot, facing `o`): (entry, x, y, yaw in radians), the same turn
/// as den.rs `stamp` (15 degree steps). The z is the caller's (the floor there).
pub fn lay_out(t: &Theme, at: At) -> Vec<(u32, f32, f32, f32)> {
    let step = std::f32::consts::PI / 12.0;
    let o = ((at.o / step).round() * step).rem_euclid(2.0 * std::f32::consts::PI);
    let (c, s) = (o.cos(), o.sin());
    t.pieces
        .iter()
        .map(|&(e, x, y, yaw)| (e, at.x + x * c - y * s, at.y + x * s + y * c, (o + yaw.to_radians()).rem_euclid(2.0 * std::f32::consts::PI)))
        .collect()
}

/// Of her pieces, the ones still where she put them (not moved, not locked, still there): what "tidy up" may take.
/// `props`: the Den's (guid, x, y, z, locked).
pub fn tidy_targets(mine: &[Mine], props: &[(u64, f32, f32, f32, bool)]) -> Vec<u64> {
    mine.iter()
        .filter(|m| {
            props.iter().any(|&(g, x, y, z, locked)| g == m.guid && !locked && (x - m.x).abs() < 0.05 && (y - m.y).abs() < 0.05 && (z - m.z).abs() < 0.05)
        })
        .map(|m| m.guid)
        .collect()
}

// ---------------------------------------------------------------- the words for her

/// Her newer words ("Jaina, <words>"), beside companion.rs `command`.
#[derive(Debug, Clone, PartialEq)]
pub enum Word {
    Talk(Talk),
    /// "help", "an idea": a suggestion (a set's preview).
    Help,
    /// "decorate": a seasonal set.
    Decorate,
    Yes,
    No,
    Tidy,
    /// "a pet", "a rabbit": the critter's index in CRITTERS, or any.
    Critter(Option<usize>),
    CrittersAway,
    Memory,
    /// "build a cozy corner": one of her sets (not a stamp: those stay dojo.rs `build_words`).
    Build(&'static str),
}

/// One of her newer words, as the whole line.
pub fn word(words: &str) -> Option<Word> {
    let low = words.to_lowercase();
    let t = low.trim().trim_end_matches(['.', '!', '?']).trim();
    let t = t.strip_prefix("please ").unwrap_or(t);
    let t = t.strip_suffix(" please").unwrap_or(t).trim_end_matches(',').trim();
    let w = match t {
        "quiet" | "be quiet" | "hush" | "shh" | "shush" | "quiet please" | "quiet mode" | "talk less" | "less talking" => Word::Talk(Talk::Quiet),
        "talk" | "you can talk" | "talk again" | "speak" | "talk normally" | "normal" => Word::Talk(Talk::Normal),
        "chatty" | "talk more" | "be chatty" | "chat" | "more talking" => Word::Talk(Talk::Chatty),
        "help" | "help me" | "help build" | "help me build" | "an idea" | "idea" | "ideas please" | "suggest" | "suggest something"
        | "surprise me" | "give me an idea" | "inspire me" => Word::Help,
        "decorate" | "decorations" | "decorate for me" | "something seasonal" | "seasonal" => Word::Decorate,
        "yes" | "yeah" | "yep" | "sure" | "ok" | "okay" | "keep it" | "keep" | "accept" | "do it" | "go ahead" | "y" => Word::Yes,
        "no" | "nope" | "no thanks" | "no thank you" | "not now" | "reject" | "take it away" | "nah" | "n" => Word::No,
        "tidy" | "tidy up" | "clean up" | "clean" | "clear up" | "clean up after yourself" | "tidy your things" => Word::Tidy,
        "pets away" | "critters away" | "no more pets" | "send the pets home" | "pets go home" | "critters go home" => Word::CrittersAway,
        "what did i build" | "what have i built" | "what do you remember" | "remember" | "memory" | "my favourite"
        | "my favorite" | "what's my favourite" | "what's my favorite" => Word::Memory,
        _ => {
            let parts: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|x| !x.is_empty()).collect();
            let filler = |x: &str| !matches!(x, "a" | "an" | "the" | "me" | "us" | "my" | "our" | "some" | "little" | "small" | "cute" | "new" | "nice" | "cozy" | "cosy");
            match parts.first().copied() {
                Some("pet") | Some("critter") | Some("pets") | Some("critters") if parts.len() == 1 => Word::Critter(None),
                Some("a") | Some("bring") | Some("get") | Some("spawn") | Some("summon") => {
                    let rest: Vec<&str> = parts[1..].iter().copied().filter(|x| filler(x)).collect();
                    match rest.first().copied() {
                        Some("pet") | Some("critter") | Some("friend") | Some("animal") if rest.len() == 1 => Word::Critter(None),
                        Some(n) if rest.len() <= 2 => match CRITTERS.iter().position(|c| c.2.contains(&n) || c.2.contains(&n.trim_end_matches('s'))) {
                            Some(i) => Word::Critter(Some(i)),
                            None => return None,
                        },
                        _ => return None,
                    }
                }
                Some("build") | Some("make") | Some("set") | Some("put") => {
                    let rest: Vec<&str> = parts[1..].iter().copied().filter(|x| filler(x) && !matches!(*x, "up" | "down" | "please")).collect();
                    let th = rest.iter().find_map(|w| theme(w))?;
                    Word::Build(th.key)
                }
                _ => return None,
            }
        }
    };
    Some(w)
}

// ---------------------------------------------------------------- the brain

#[derive(Debug, Clone)]
pub struct Brain {
    pub rng: Rng,
    pub talk: Talk,
    pub needs: Needs,
    pub joy: f32,
    pub energy: f32,
    pub memory: Memory,
    pub doing: Doing,
    pub question: Option<Question>,
    pub mine: Vec<Mine>,
    pub critters: Vec<Critter>,
    /// She stopped suggesting by herself (two unasked suggestions sent away); "Jaina, help" still works.
    pub offers_off: bool,
    pending: Vec<(Event, u64)>,
    arrived: u64,
    last_think: u64,
    next_think: u64,
    next_line: u64,
    next_emote: u64,
    next_idle: u64,
    next_offer: u64,
    explore_until: u64,
    lines_said: u32,
    offers_made: u32,
    unasked_rejects: u32,
    critter_n: u32,
    /// The piece her last reaction was about ("Another {fav}?").
    last_entry: Option<u32>,
    last_player: Option<At>,
    last_move: u64,
    /// The last line said per key (no repeats running).
    last_pick: BTreeMap<String, usize>,
    lines: Vec<(String, String)>,
}

impl Default for Brain {
    fn default() -> Self {
        Brain::new(1)
    }
}

fn d2(a: At, b: At) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

impl Brain {
    pub fn new(seed: u64) -> Brain {
        Brain {
            rng: Rng::new(seed),
            talk: Talk::Normal,
            needs: Needs::default(),
            joy: 0.5,
            energy: 1.0,
            memory: Memory::default(),
            doing: Doing::Following,
            question: None,
            mine: Vec::new(),
            critters: Vec::new(),
            offers_off: false,
            pending: Vec::new(),
            arrived: 0,
            last_think: 0,
            next_think: 0,
            next_line: 0,
            next_emote: 0,
            next_idle: 0,
            next_offer: 0,
            explore_until: 0,
            lines_said: 0,
            offers_made: 0,
            unasked_rejects: 0,
            critter_n: 0,
            last_entry: None,
            last_player: None,
            last_move: 0,
            last_pick: BTreeMap::new(),
            lines: parse_lines(LINES_TEXT),
        }
    }

    /// A new visit (login): memory of the last one goes, the clocks start now; how she talks is kept.
    pub fn arrive(&mut self, ms: u64) {
        let (talk, rng, mine) = (self.talk, self.rng.clone(), std::mem::take(&mut self.mine));
        *self = Brain { talk, rng, mine, ..Brain::new(0) };
        self.arrived = ms;
        self.last_think = ms;
        self.last_move = ms;
        self.next_line = ms + WARMUP_MS;
        self.next_offer = ms + OFFER_FIRST_MS;
        self.next_idle = ms + IDLE_GAP_MS;
    }

    pub fn mood(&self) -> Mood {
        if self.energy < 0.3 {
            Mood::Sleepy
        } else if self.joy > 0.8 {
            Mood::Excited
        } else if self.needs.curiosity > 0.8 {
            Mood::Curious
        } else if self.joy > 0.62 {
            Mood::Cheerful
        } else {
            Mood::Calm
        }
    }

    /// One of the lines for `key`, filled in, never the same as last time when there are others; "" for a key the
    /// file hasn't.
    pub fn line(&mut self, key: &str, with: &[(&str, &str)]) -> String {
        let all: Vec<usize> = self.lines.iter().enumerate().filter(|(_, (k, _))| k == key).map(|(i, _)| i).collect();
        if all.is_empty() {
            return String::new();
        }
        let last = self.last_pick.get(key).copied();
        let choices: Vec<usize> = if all.len() > 1 { all.iter().copied().filter(|i| Some(*i) != last).collect() } else { all.clone() };
        let i = choices[self.rng.below(choices.len())];
        self.last_pick.insert(key.to_string(), i);
        fill(&self.lines[i].1, with)
    }

    /// Unasked words allowed now.
    pub fn may_talk(&self, ms: u64) -> bool {
        let cap = match self.talk {
            Talk::Quiet => return false,
            Talk::Normal => LINES_CAP_NORMAL,
            Talk::Chatty => LINES_CAP_CHATTY,
        };
        ms >= self.next_line && self.lines_said < cap
    }

    fn said(&mut self, ms: u64) {
        self.lines_said += 1;
        self.next_line = ms + if self.talk == Talk::Chatty { LINE_GAP_CHATTY_MS } else { LINE_GAP_NORMAL_MS };
    }

    /// "Jaina, quiet" and the others: the line she answers with.
    pub fn set_talk(&mut self, t: Talk, ms: u64) -> String {
        self.talk = t;
        // a change of mind: her next unasked line no sooner than a gap from now
        self.next_line = self.next_line.max(ms + if t == Talk::Chatty { LINE_GAP_CHATTY_MS / 2 } else { LINE_GAP_NORMAL_MS / 2 });
        if t != Talk::Quiet {
            self.offers_off = false;
            self.unasked_rejects = 0;
        }
        self.line(&format!("talk.{}", t.word()), &[])
    }

    /// Something happened: remembered now, reacted to at her next thought (if still fresh).
    pub fn feed(&mut self, ev: Event, ms: u64) {
        match &ev {
            Event::Placed { entry, n } => {
                *self.memory.placed.entry(*entry).or_insert(0) += 1;
                self.memory.pieces += *n as u32;
                self.memory.recent.push(ms);
                self.memory.recent.retain(|t| ms.saturating_sub(*t) <= OFFER_RECENT_MS);
                self.needs.helping = (self.needs.helping + 0.12).min(1.0);
                self.joy = (self.joy + 0.05).min(1.0);
                self.energy = (self.energy + 0.1).min(1.0);
            }
            Event::Deleted { n } => {
                self.memory.deleted += *n as u32;
                if *n >= 5 {
                    self.joy = (self.joy - 0.05).max(0.0);
                }
            }
            Event::Saved => self.memory.saves += 1,
            Event::Grew => self.joy = (self.joy + 0.1).min(1.0),
            Event::FlyStart => self.memory.flights += 1,
            Event::FlyEnd => {}
            Event::FightStart => self.memory.fights += 1,
            Event::FightEnd { won } => self.joy = if *won { (self.joy + 0.1).min(1.0) } else { self.joy },
            Event::GuestArrived => {
                self.memory.guests += 1;
                self.joy = (self.joy + 0.2).min(1.0);
                self.needs.company = 0.0;
            }
            Event::GuestLeft => {}
            Event::Built => {
                self.memory.builds += 1;
                self.joy = (self.joy + 0.15).min(1.0);
            }
        }
        self.energy = (self.energy + 0.05).min(1.0);
        self.pending.push((ev, ms));
        let n = self.pending.len();
        self.pending.drain(..n.saturating_sub(8));
    }

    /// A Den change (world.rs `apply`): pieces made (guid, entry) and how many went; hers don't count as yours.
    pub fn saw_change(&mut self, created: &[(u64, u32)], destroyed: usize, ms: u64) {
        let yours: Vec<u32> = created.iter().filter(|(g, _)| !self.is_mine(*g)).map(|(_, e)| *e).collect();
        if let Some(&entry) = yours.first() {
            self.feed(Event::Placed { entry, n: yours.len() }, ms);
        }
        if destroyed > 0 {
            self.feed(Event::Deleted { n: destroyed }, ms);
        }
    }

    pub fn is_mine(&self, guid: u64) -> bool {
        self.mine.iter().any(|m| m.guid == guid)
    }

    /// Pieces she put down (a kept set, her build): hers until you move them.
    pub fn adopt(&mut self, pieces: &[(u64, f32, f32, f32)]) {
        for &(guid, x, y, z) in pieces {
            if !self.is_mine(guid) {
                self.mine.push(Mine { guid, x, y, z });
            }
        }
    }

    /// Her pieces still in the Den (`alive`: is that guid still a prop).
    pub fn forget_gone(&mut self, alive: impl Fn(u64) -> bool) {
        self.mine.retain(|m| alive(m.guid));
    }

    // ---- suggestions

    /// Which set to suggest: in season first, then one with your favourite piece, never one sent away twice; the
    /// seeded pick among the best. None: every one was sent away.
    pub fn pick_theme(&mut self, month: u32, seasonal_only: bool) -> Option<&'static Theme> {
        let fav = self.memory.favourite().map(|f| f.0);
        let scored: Vec<(&'static Theme, u32)> = THEMES
            .iter()
            .filter(|t| self.memory.sent_away.iter().filter(|k| **k == t.key).count() < 2)
            .filter(|t| !seasonal_only || in_season(t, month))
            .map(|t| {
                let mut s: u32 = 1;
                if in_season(t, month) {
                    s += 4;
                }
                if !t.months.is_empty() && !in_season(t, month) {
                    s = 0; // out of season: not by herself
                }
                if fav.is_some_and(|f| t.pieces.iter().any(|p| p.0 == f)) {
                    s += 2;
                }
                if self.memory.kept.contains(&t.key) || self.memory.sent_away.contains(&t.key) {
                    s = s.saturating_sub(2).max(if s > 0 { 1 } else { 0 });
                }
                (t, s)
            })
            .filter(|(_, s)| *s > 0)
            .collect();
        let best = scored.iter().map(|x| x.1).max()?;
        let top: Vec<&'static Theme> = scored.iter().filter(|x| x.1 == best).map(|x| x.0).collect();
        Some(top[self.rng.below(top.len())])
    }

    /// The preview is shown (world.rs): she waits for yes or no. `asked`: you asked for it.
    pub fn offered(&mut self, theme: &'static str, at: At, guids: Vec<u64>, asked: bool, ms: u64) {
        self.question = Some(Question::Offer { theme, at, guids, asked, until: ms + OFFER_MS });
        self.doing = Doing::Offering;
        if !asked {
            self.offers_made += 1;
            self.next_offer = ms + OFFER_GAP_MS;
        }
        self.needs.helping = 0.0;
    }

    /// The line for a suggestion (in season or not).
    pub fn offer_line(&mut self, t: &Theme, month: u32) -> String {
        let key = if in_season(t, month) { "offer.season" } else { "offer" };
        self.line(key, &[("theme", t.title)])
    }

    /// "Jaina, yes": the open question taken (an offer to keep, a tidy to do).
    pub fn yes(&mut self) -> Option<Question> {
        let q = self.question.take()?;
        if let Question::Offer { theme, .. } = &q {
            self.memory.kept.push(theme);
            self.joy = (self.joy + 0.15).min(1.0);
        }
        self.doing = Doing::Following;
        Some(q)
    }

    /// "Jaina, no": the open question dropped. A suggestion she made by herself sent away twice: no more of those.
    pub fn no(&mut self) -> Option<Question> {
        let q = self.question.take()?;
        if let Question::Offer { theme, asked, .. } = &q {
            self.memory.sent_away.push(theme);
            if !asked {
                self.unasked_rejects += 1;
                if self.unasked_rejects >= 2 {
                    self.offers_off = true;
                }
            }
        }
        self.doing = Doing::Following;
        Some(q)
    }

    /// "Tidy up?": waits for yes.
    pub fn ask_tidy(&mut self, guids: Vec<u64>, ms: u64) {
        self.question = Some(Question::Tidy { guids, until: ms + ASK_MS });
    }

    // ---- critters

    /// A critter at `at` (one of CRITTERS, or her pick: the seeded one); Err: the line key (full).
    pub fn add_critter(&mut self, which: Option<usize>, at: At, ms: u64) -> Result<Critter, &'static str> {
        if self.critters.len() >= MAX_CRITTERS {
            return Err("critter.full");
        }
        let i = which.unwrap_or_else(|| self.rng.below(CRITTERS.len())).min(CRITTERS.len() - 1);
        self.critter_n += 1;
        let c = Critter { guid: critter_guid(CRITTERS[i].0, self.critter_n), entry: CRITTERS[i].0, home: at, at, next: ms + self.rng.between(CRITTER_STEP_MIN_MS, CRITTER_STEP_MAX_MS) };
        self.critters.push(c);
        Ok(c)
    }

    /// The critters' steps due now: (guid, from, to, milliseconds), a short walk somewhere near where each came,
    /// pushed out of props (`blocks`: x, y, radius).
    pub fn critter_steps(&mut self, ms: u64, blocks: &[(f32, f32, f32)]) -> Vec<(u64, At, At, u32)> {
        let mut out = Vec::new();
        for i in 0..self.critters.len() {
            if ms < self.critters[i].next {
                continue;
            }
            let a = self.rng.unit() * std::f32::consts::TAU;
            let r = 0.5 + self.rng.unit() * (CRITTER_ROAM - 0.5);
            let c = self.critters[i];
            let mut to = At { x: c.home.x + a.cos() * r, y: c.home.y + a.sin() * r, z: c.home.z, o: 0.0 };
            for &(bx, by, br) in blocks {
                let d = ((to.x - bx).powi(2) + (to.y - by).powi(2)).sqrt();
                if d < br + 0.3 {
                    let (ux, uy) = if d > 1e-3 { ((to.x - bx) / d, (to.y - by) / d) } else { (1.0, 0.0) };
                    to = At { x: bx + ux * (br + 0.3), y: by + uy * (br + 0.3), ..to };
                }
            }
            to.o = (to.y - c.at.y).atan2(to.x - c.at.x).rem_euclid(std::f32::consts::TAU);
            let t = ((d2(c.at, to) / 1.5) * 1000.0).max(300.0) as u32;
            out.push((c.guid, c.at, to, t));
            self.critters[i].at = to;
            self.critters[i].next = ms + t as u64 + self.rng.between(CRITTER_STEP_MIN_MS, CRITTER_STEP_MAX_MS);
        }
        out
    }

    /// The critters go: their guids.
    pub fn critters_away(&mut self) -> Vec<u64> {
        self.critters.drain(..).map(|c| c.guid).collect()
    }

    // ---- what she remembers, said

    /// "Jaina, what did I build?" (`name`: a catalog entry's name).
    pub fn memory_line(&mut self, name: impl Fn(u32) -> String) -> String {
        let n = self.memory.pieces.to_string();
        if self.memory.pieces == 0 {
            return self.line("memory.none", &[]);
        }
        match self.memory.favourite() {
            Some((e, _)) => {
                let fav = name(e);
                self.line("memory", &[("n", &n), ("fav", &fav)])
            }
            None => self.line("memory.nofav", &[("n", &n)]),
        }
    }

    /// A line's `{fav}`: the name of the piece her reaction was about (else your favourite), from the catalog.
    pub fn name_in(&self, line: &str, name: impl Fn(u32) -> String) -> String {
        match self.last_entry.or(self.memory.favourite().map(|f| f.0)) {
            Some(e) if line.contains("{fav}") => fill(line, &[("fav", &name(e))]),
            _ => line.replace("{fav}", "one of those"),
        }
    }

    /// For the Den panel (`wuden jaina`) and the heartbeat: talk, doing, mood, the open question, her pieces, critters.
    pub fn ui_line(&self) -> String {
        let q = match &self.question {
            Some(Question::Offer { theme, .. }) => format!("offer={theme};ask=offer"),
            Some(Question::Tidy { guids, .. }) => format!("offer=-;ask=tidy{}", guids.len()),
            None => "offer=-;ask=-".into(),
        };
        let fav = self.memory.favourite().map(|f| f.0.to_string()).unwrap_or_else(|| "-".into());
        format!("talk={};doing={};mood={};{q};mine={};critters={};placed={};fav={fav}", self.talk.word(), self.doing.word(), self.mood().word(), self.mine.len(), self.critters.len(), self.memory.pieces)
    }

    // ---- thinking

    /// One thought (at most every THINK_MS): what she does now. Nothing, mostly.
    pub fn think(&mut self, s: &Sense, blocks: &[(f32, f32, f32)]) -> Vec<Act> {
        let ms = s.ms;
        if ms < self.next_think {
            return Vec::new();
        }
        self.next_think = ms + THINK_MS;
        let dt = (ms.saturating_sub(self.last_think) as f32 / 1000.0).clamp(0.0, 5.0);
        self.last_think = ms;
        // you moving or not
        if self.last_player.map_or(true, |p| d2(p, s.player) > 0.4) {
            // back from a long still while: she perks up
            if ms.saturating_sub(self.last_move) >= STILL_MS * 4 {
                self.energy = self.energy.max(0.5);
            }
            self.last_move = ms;
            self.last_player = Some(s.player);
        }
        let still = ms.saturating_sub(self.last_move);
        // needs grow; mood drifts back to calm; standing still a long while makes her sleepy
        self.needs.company = (self.needs.company + dt / 240.0).min(1.0);
        self.needs.curiosity = (self.needs.curiosity + dt / 150.0).min(1.0);
        self.needs.helping = (self.needs.helping - dt / 600.0).max(0.0);
        self.needs.rest = if still >= STILL_MS { (self.needs.rest + dt / 300.0).min(1.0) } else { (self.needs.rest - dt / 60.0).max(0.0) };
        self.joy += (0.5 - self.joy) * (dt / 600.0).min(1.0);
        self.energy = if still >= STILL_MS * 4 { (self.energy - dt / 600.0).max(0.0) } else { (self.energy + dt / 120.0).min(1.0) };
        // what she is doing
        self.doing = if s.building {
            Doing::Building
        } else if s.flying {
            Doing::Flying
        } else if self.question.as_ref().is_some_and(|q| matches!(q, Question::Offer { .. })) {
            Doing::Offering
        } else if !s.following {
            Doing::Waiting
        } else if ms < self.explore_until {
            Doing::Exploring
        } else {
            Doing::Following
        };
        let mut acts = Vec::new();
        // an unanswered question goes
        let expired = match &self.question {
            Some(Question::Offer { until, .. }) | Some(Question::Tidy { until, .. }) => ms >= *until,
            None => false,
        };
        if expired {
            if let Some(Question::Offer { guids, theme, asked, .. }) = self.question.take() {
                acts.push(Act::Withdraw(guids));
                self.memory.sent_away.push(theme);
                if !asked {
                    self.unasked_rejects += 1;
                    self.offers_off |= self.unasked_rejects >= 2;
                }
                if self.may_talk(ms) {
                    let l = self.line("offer.expired", &[]);
                    self.said(ms);
                    acts.push(Act::Line(l));
                }
            }
            self.question = None;
            self.doing = Doing::Following;
        }
        self.pending.retain(|(_, t)| ms.saturating_sub(*t) <= REACT_FRESH_MS);
        // the choices, scored
        let react = self.best_reaction(ms);
        let explore = if s.following && !s.walking && !s.building && !s.flying && self.question.is_none() && ms >= self.explore_until
            && ms.saturating_sub(self.arrived) >= EXPLORE_AFTER_MS && still >= STILL_MS
        {
            self.needs.curiosity * 0.6
        } else {
            0.0
        };
        let idle = if still >= STILL_MS && ms >= self.next_idle && !s.building && ms >= self.next_emote {
            0.2 + 0.2 * self.needs.company.max(self.needs.rest)
        } else {
            0.0
        };
        let offers_cap = match self.talk {
            Talk::Quiet => 0,
            Talk::Normal => OFFERS_CAP_NORMAL,
            Talk::Chatty => OFFERS_CAP_CHATTY,
        };
        let offer = if !self.offers_off && self.offers_made < offers_cap && ms >= self.next_offer && self.question.is_none() && !s.building
            && !s.flying && self.may_talk(ms) && self.memory.recent_since(ms.saturating_sub(OFFER_RECENT_MS)) >= OFFER_AFTER_PLACED
            && still >= 4_000
        {
            0.3 + 0.5 * self.needs.helping
        } else {
            0.0
        };
        let best = [react.as_ref().map_or(0.0, |r| r.0), explore, idle, offer].into_iter().fold(0.0f32, f32::max);
        if best < 0.2 {
            return acts;
        }
        if react.as_ref().is_some_and(|r| r.0 == best) {
            let (_, ev) = react.unwrap();
            self.pending.clear();
            acts.extend(self.react(&ev, ms));
        } else if offer == best {
            if let Some(t) = self.pick_theme(s.month, false) {
                acts.push(Act::Offer(t.key));
            }
        } else if explore == best {
            let a = self.rng.unit() * std::f32::consts::TAU;
            let r = 3.0 + self.rng.unit() * (EXPLORE_RADIUS - 3.0);
            let mut to = At { x: s.player.x + a.cos() * r, y: s.player.y + a.sin() * r, z: s.player.z, o: 0.0 };
            // inside the Den: pulled toward its middle
            let dm = d2(to, s.mid);
            let max = (s.reach - 1.5).max(1.0);
            if dm > max {
                to = At { x: s.mid.x + (to.x - s.mid.x) / dm * max, y: s.mid.y + (to.y - s.mid.y) / dm * max, ..to };
            }
            for &(bx, by, br) in blocks {
                let d = ((to.x - bx).powi(2) + (to.y - by).powi(2)).sqrt();
                if d < br + 0.6 {
                    let (ux, uy) = if d > 1e-3 { ((to.x - bx) / d, (to.y - by) / d) } else { (1.0, 0.0) };
                    to = At { x: bx + ux * (br + 0.6), y: by + uy * (br + 0.6), ..to };
                }
            }
            to.o = (to.y - s.her.y).atan2(to.x - s.her.x).rem_euclid(std::f32::consts::TAU);
            self.explore_until = ms + EXPLORE_MS;
            self.needs.curiosity = 0.0;
            self.doing = Doing::Exploring;
            acts.push(Act::Wander(to));
            if self.rng.chance(0.1) && self.may_talk(ms) {
                let l = self.line("explore", &[]);
                self.said(ms);
                acts.push(Act::Line(l));
            }
        } else if idle == best {
            self.next_idle = ms + IDLE_GAP_MS;
            self.next_emote = ms + EMOTE_GAP_MS;
            self.needs.company *= 0.5;
            acts.push(Act::Emote(self.mood().gesture()));
            if self.rng.chance(0.25) && self.may_talk(ms) {
                let key = format!("idle.{}", self.mood().word());
                let l = self.line(&key, &[]);
                self.said(ms);
                acts.push(Act::Line(l));
            }
        }
        acts
    }

    /// The freshest thing worth reacting to and how much (fresher and bigger: more).
    fn best_reaction(&self, ms: u64) -> Option<(f32, Event)> {
        if ms < self.next_emote && !self.may_talk(ms) {
            return None;
        }
        self.pending
            .iter()
            .map(|(ev, t)| {
                let base = match ev {
                    Event::GuestArrived => 0.9,
                    Event::FightEnd { .. } => 0.8,
                    Event::Placed { n, .. } if *n >= LOTS => 0.6,
                    Event::Placed { .. } if self.memory.pieces <= 1 => 0.7,
                    Event::Placed { .. } => 0.35,
                    Event::FlyStart | Event::Grew | Event::Built => 0.6,
                    Event::Saved => 0.5,
                    Event::FightStart | Event::GuestLeft | Event::FlyEnd => 0.4,
                    Event::Deleted { n } if *n >= 5 => 0.45,
                    Event::Deleted { .. } => 0.25,
                };
                let fresh = 1.0 - (ms.saturating_sub(*t) as f32 / REACT_FRESH_MS as f32).min(1.0);
                (base * (0.5 + 0.5 * fresh), ev.clone())
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Her reaction: a gesture (when one is due), and now and then a line (when words are allowed).
    fn react(&mut self, ev: &Event, ms: u64) -> Vec<Act> {
        let fav_name = |b: &Brain, e: u32| b.memory.placed.get(&e).copied().unwrap_or(0);
        let (emote, key, chance): (Option<u32>, &str, f32) = match ev {
            Event::Placed { n, .. } if *n >= LOTS => (Some(EXCLAMATION), "place.lots", 0.6),
            Event::Placed { .. } if self.memory.pieces <= 1 => (Some(APPLAUD), "place.first", 1.0),
            Event::Placed { entry, .. } if fav_name(self, *entry) == 3 => (Some(LAUGH), "place.again", 0.5),
            Event::Placed { .. } => (if self.rng.chance(0.4) { Some(if self.rng.chance(0.5) { APPLAUD } else { POINT }) } else { None }, "place", 0.15),
            Event::Deleted { n } if *n >= 5 => (Some(EXCLAMATION), "delete.lots", 0.5),
            Event::Deleted { .. } => (if self.rng.chance(0.3) { Some(SHY) } else { None }, "delete", 0.1),
            Event::Saved => (Some(YES), "save", 0.3),
            Event::Grew => (Some(CHEER), "grow", 0.5),
            Event::FlyStart => (Some(CHEER), "fly", 0.5),
            Event::FlyEnd => (None, "land", 0.2),
            Event::FightStart => (Some(SALUTE), "fight", 0.3),
            Event::FightEnd { won: true } => (Some(CHEER), "fight.won", 0.6),
            Event::FightEnd { won: false } => (Some(TALK), "fight.lost", 0.5),
            Event::GuestArrived => (Some(WAVE), "guest", 0.8),
            Event::GuestLeft => (Some(WAVE), "guest.left", 0.4),
            // dojo.rs already says "There." when a build is done: a bow only
            Event::Built => (Some(BOW), "built", 0.0),
        };
        let mut acts = Vec::new();
        if let Some(e) = emote.filter(|_| ms >= self.next_emote) {
            self.next_emote = ms + EMOTE_GAP_MS;
            acts.push(Act::Emote(e));
        }
        if chance > 0.0 && self.may_talk(ms) && self.rng.chance(chance) {
            if let Event::Placed { entry, .. } = ev {
                self.last_entry = Some(*entry);
            }
            // "{fav}" stays for world.rs: the piece's name (Brain::name_in), which only the catalog knows
            let l = self.line(key, &[]);
            self.said(ms);
            acts.push(Act::Line(l));
        }
        acts
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> At {
        At { x, y, z: 0.0, o: 0.0 }
    }

    fn sense(ms: u64, p: At) -> Sense {
        Sense { ms, player: p, her: at(p.x - 2.0, p.y), walking: false, following: true, flying: false, building: false, mid: at(0.0, 0.0), reach: 22.0, month: 11 }
    }

    const BRANDS: [&str; 4] = ["world of warcraft", "warcraft", "lich king", "blizzard"];

    #[test]
    fn the_same_seed_the_same_choices() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        let xs: Vec<u64> = (0..20).map(|_| a.next_u64()).collect();
        let ys: Vec<u64> = (0..20).map(|_| b.next_u64()).collect();
        assert_eq!(xs, ys);
        assert_ne!(Rng::new(8).next_u64(), xs[0]);
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
            assert!(r.below(3) < 3);
            let x = r.between(5, 9);
            assert!((5..=9).contains(&x));
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn every_line_is_short_plain_and_every_key_has_one() {
        let lines = parse_lines(LINES_TEXT);
        assert!(lines.len() >= KEYS.len());
        for (k, l) in &lines {
            assert!(l.chars().count() <= 120, "{k}: {l}");
            let low = l.to_lowercase();
            assert!(BRANDS.iter().all(|b| !low.contains(b)), "{k}");
            assert!(!low.split(|c: char| !c.is_alphanumeric()).any(|w| w == "wow"), "{k}");
            assert!(!l.contains('|') && !l.contains("http"), "{k}");
            assert!(!l.chars().any(|c| c.is_ascii_digit()), "{k}: no numbers in her words");
            assert!(!low.contains("sign in"), "{k}: Log in, never sign in");
        }
        for k in KEYS {
            assert!(lines.iter().any(|(x, _)| x == k), "no line for {k}");
        }
        for (k, _) in &lines {
            assert!(KEYS.contains(&k.as_str()), "a line nobody says: {k}");
        }
        assert_eq!(parse_lines("# c\n\n a | b \nbad line\n|x\n"), vec![("a".to_string(), "b".to_string())]);
        assert_eq!(fill("a {theme} for {n}", &[("theme", "picnic"), ("n", "two")]), "a picnic for two");
    }

    #[test]
    fn lines_vary_and_never_repeat_running() {
        let mut b = Brain::new(3);
        let mut last = String::new();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..30 {
            let l = b.line("place", &[]);
            assert!(!l.is_empty() && l != last, "{l}");
            seen.insert(l.clone());
            last = l;
        }
        assert!(seen.len() >= 3);
        assert_eq!(b.line("no.such.key", &[]), "");
        assert_eq!(b.line("offer.none", &[]), b.line("offer.none", &[]), "one line: the same one");
    }

    #[test]
    fn she_waits_before_talking_and_keeps_quiet_when_told() {
        let mut b = Brain::new(1);
        b.arrive(1_000);
        assert!(!b.may_talk(1_000 + WARMUP_MS - 1), "the first minute and a half: no unasked words");
        assert!(b.may_talk(1_000 + WARMUP_MS));
        // a reaction inside the warm-up: a gesture, no words
        b.feed(Event::GuestArrived, 2_000);
        let acts = b.think(&sense(2_000, at(0.0, 0.0)), &[]);
        assert_eq!(acts, vec![Act::Emote(WAVE)]);
        // quiet: gestures only, ever
        let t = b.set_talk(Talk::Quiet, 200_000);
        assert!(t.contains("quiet"));
        b.feed(Event::GuestArrived, 400_000);
        let acts = b.think(&sense(400_000, at(0.0, 0.0)), &[]);
        assert_eq!(acts, vec![Act::Emote(WAVE)]);
        assert!(!b.may_talk(10_000_000));
        // back to normal: words again (after a gap)
        b.set_talk(Talk::Normal, 500_000);
        assert!(!b.may_talk(500_000) && b.may_talk(500_000 + LINE_GAP_NORMAL_MS));
    }

    #[test]
    fn unasked_words_are_rare() {
        let mut b = Brain::new(5);
        b.arrive(0);
        let mut lines = 0;
        let mut ms = WARMUP_MS;
        // a busy hour: you place something every 3 seconds
        while ms < WARMUP_MS + 3_600_000 {
            b.feed(Event::Placed { entry: 2413, n: 1 }, ms);
            let p = at((ms / 3000 % 7) as f32, 0.0);
            for a in b.think(&sense(ms, p), &[]) {
                if matches!(a, Act::Line(_)) {
                    lines += 1;
                }
                if let Act::Offer(k) = a {
                    b.offered(theme_by_key(k).unwrap().key, p, vec![1], false, ms);
                    b.no();
                }
            }
            ms += 3_000;
        }
        assert!(lines >= 2 && lines as u32 <= LINES_CAP_NORMAL, "{lines}");
        // and gestures no closer than EMOTE_GAP_MS
        let mut b = Brain::new(5);
        b.arrive(0);
        let mut last = None;
        for i in 0..200u64 {
            let ms = i * 1000;
            b.feed(Event::Placed { entry: 2413, n: 1 }, ms);
            for a in b.think(&sense(ms, at(i as f32 % 3.0, 0.0)), &[]) {
                if matches!(a, Act::Emote(_)) {
                    if let Some(l) = last {
                        assert!(ms - l >= EMOTE_GAP_MS, "{ms} {l}");
                    }
                    last = Some(ms);
                }
            }
        }
    }

    #[test]
    fn she_remembers_your_favourite_and_not_her_own_pieces() {
        let mut b = Brain::new(1);
        b.arrive(0);
        assert_eq!(b.memory.favourite(), None);
        b.saw_change(&[(10, 2413)], 0, 1);
        b.saw_change(&[(11, 180765)], 0, 2);
        assert_eq!(b.memory.favourite(), None, "once each: no favourite yet");
        b.saw_change(&[(12, 2413)], 0, 3);
        assert_eq!(b.memory.favourite(), Some((2413, 2)));
        b.adopt(&[(50, 1.0, 2.0, 0.0), (51, 1.0, 3.0, 0.0)]);
        b.saw_change(&[(50, 182093), (51, 182093)], 0, 4);
        assert_eq!(b.memory.pieces, 3, "her own set isn't yours");
        b.saw_change(&[], 2, 5);
        assert_eq!(b.memory.deleted, 2);
        let l = b.memory_line(|e| if e == 2413 { "Wooden Chair".into() } else { "?".into() });
        assert!(l.contains("Wooden Chair"), "{l}");
        let mut fresh = Brain::new(1);
        assert!(!fresh.memory_line(|_| String::new()).is_empty());
        // a new visit: memory starts again, how she talks and her pieces are kept
        b.set_talk(Talk::Chatty, 0);
        b.arrive(100);
        assert_eq!((b.memory.pieces, b.talk, b.mine.len()), (0, Talk::Chatty, 2));
        b.forget_gone(|g| g == 50);
        assert_eq!(b.mine.len(), 1);
    }

    #[test]
    fn she_suggests_while_you_build_and_stops_when_sent_away() {
        let mut b = Brain::new(9);
        b.arrive(0);
        let mut offers = Vec::new();
        let mut ms = OFFER_FIRST_MS - 30_000;
        let p = at(0.0, 0.0);
        while ms < OFFER_FIRST_MS + 3 * OFFER_GAP_MS {
            // you build for a while, then pause
            if (ms / 1000) % 30 < 10 && (ms / 1000) % 3 == 0 {
                b.feed(Event::Placed { entry: 19429, n: 1 }, ms);
            }
            for a in b.think(&sense(ms, p), &[]) {
                if let Act::Offer(k) = a {
                    offers.push((ms, k));
                    b.offered(k, p, vec![77], false, ms);
                    assert!(b.ui_line().contains(&format!("offer={k}")));
                    assert!(b.no().is_some());
                }
            }
            ms += 1000;
        }
        assert_eq!(offers.len(), 2, "two unasked, both sent away: she stops ({offers:?})");
        assert!(offers[0].0 >= OFFER_FIRST_MS && offers[1].0 - offers[0].0 >= OFFER_GAP_MS);
        assert!(b.offers_off);
        assert!(theme_by_key(offers[0].1).unwrap().months.is_empty() || in_season(theme_by_key(offers[0].1).unwrap(), 11), "nothing out of season by herself");
        // asking her again turns them back on
        b.set_talk(Talk::Normal, ms);
        assert!(!b.offers_off);
    }

    #[test]
    fn an_offer_waits_a_minute_then_goes() {
        let mut b = Brain::new(2);
        b.arrive(0);
        let p = at(0.0, 0.0);
        b.offered("corner", p, vec![5, 6], true, 1000);
        assert!(matches!(b.question, Some(Question::Offer { .. })));
        assert!(b.think(&sense(1000 + OFFER_MS - 1000, p), &[]).iter().all(|a| !matches!(a, Act::Withdraw(_))));
        let acts = b.think(&sense(1000 + OFFER_MS, p), &[]);
        assert!(acts.contains(&Act::Withdraw(vec![5, 6])), "{acts:?}");
        assert!(b.question.is_none() && !b.offers_off, "asked for: unanswered doesn't turn her suggestions off");
        assert!(b.yes().is_none() && b.no().is_none(), "nothing open");
        b.offered("lights", p, vec![1], true, 2000);
        assert!(matches!(b.yes(), Some(Question::Offer { theme: "lights", .. })));
        assert_eq!(b.memory.kept, vec!["lights"]);
    }

    #[test]
    fn themes_in_season_and_never_one_sent_away_twice() {
        let mut b = Brain::new(4);
        for _ in 0..20 {
            let t = b.pick_theme(12, false).unwrap();
            assert!(t.months.is_empty() || in_season(t, 12), "{}", t.key);
        }
        // in December the winter tree comes first (in season: the best score)
        assert_eq!(b.pick_theme(12, false).unwrap().key, "winter");
        assert_eq!(b.pick_theme(12, true).unwrap().key, "winter");
        assert!(b.pick_theme(7, true).unwrap().key == "picnic");
        b.memory.sent_away.extend(["winter", "winter"]);
        assert_ne!(b.pick_theme(12, false).unwrap().key, "winter");
        assert_eq!(b.pick_theme(12, true), None, "the only one in season was sent away twice");
        // your favourite piece pulls its set up
        let mut b = Brain::new(4);
        b.memory.placed.insert(179967, 5);
        assert_eq!(b.pick_theme(6, false).unwrap().key, "picnic", "in season beats the favourite");
        assert_eq!(b.pick_theme(4, false).unwrap().key, "flowers");
        let mut b = Brain::new(4);
        b.memory.placed.insert(179967, 5);
        assert_eq!(b.pick_theme(1, false).map(|t| t.key), Some("winter"));
        b.memory.sent_away.extend(["winter", "winter"]);
        assert_eq!(b.pick_theme(1, false).unwrap().key, "stall", "the favourite (a barrel) next");
    }

    #[test]
    fn sets_laid_out_in_front_and_turned() {
        let t = theme_by_key("lights").unwrap();
        let p = lay_out(t, At { x: 10.0, y: 10.0, z: 0.0, o: 0.0 });
        assert_eq!(p.len(), 4);
        assert!((p[0].1 - 10.0).abs() < 1e-4 && (p[0].2 - 8.4).abs() < 1e-4);
        let q = lay_out(t, At { x: 10.0, y: 10.0, z: 0.0, o: std::f32::consts::FRAC_PI_2 });
        assert!((q[0].1 - 11.6).abs() < 1e-3 && (q[0].2 - 10.0).abs() < 1e-3, "{:?}", q[0]);
        assert!((q[0].3 - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        for th in THEMES {
            assert!(th.pieces.len() >= 3 && th.pieces.len() <= 6, "{}: a small set", th.key);
            assert!(th.pieces.iter().all(|p| p.1.abs() <= 3.0 && p.2.abs() <= 3.0), "{}: close together", th.key);
            assert!(th.months.iter().all(|m| (1..=12).contains(m)));
            let low = th.title.to_lowercase();
            assert!(BRANDS.iter().all(|b| !low.contains(b)));
            assert_eq!(theme(th.key).unwrap().key, th.key);
            for w in th.words {
                assert_eq!(theme(w).unwrap().key, th.key, "{w}: one set per word");
            }
        }
        assert_eq!(theme("Winter").unwrap().key, "winter");
        assert_eq!(theme("garden"), None, "the stamps stay stamps");
    }

    #[test]
    fn tidy_takes_only_her_untouched_pieces() {
        let mine = [Mine { guid: 1, x: 1.0, y: 1.0, z: 0.0 }, Mine { guid: 2, x: 2.0, y: 2.0, z: 0.0 }, Mine { guid: 3, x: 3.0, y: 3.0, z: 0.0 }, Mine { guid: 4, x: 4.0, y: 4.0, z: 0.0 }];
        let props = [(1, 1.0, 1.0, 0.0, false), (2, 2.5, 2.0, 0.0, false), (3, 3.0, 3.0, 0.0, true), (9, 4.0, 4.0, 0.0, false)];
        assert_eq!(tidy_targets(&mine, &props), vec![1], "moved: yours; locked: left; gone: nothing; yours: never");
        let mut b = Brain::new(1);
        b.ask_tidy(vec![1], 0);
        assert!(b.ui_line().contains("ask=tidy1"));
        assert!(b.think(&sense(ASK_MS, at(0.0, 0.0)), &[]).iter().all(|a| !matches!(a, Act::Withdraw(_))));
        assert!(b.question.is_none(), "a tidy unanswered just lapses");
    }

    #[test]
    fn critters_within_limits_near_where_they_came() {
        let mut b = Brain::new(6);
        let home = At { x: 5.0, y: 5.0, z: 1.0, o: 0.0 };
        let a = b.add_critter(Some(0), home, 0).unwrap();
        assert_eq!((a.entry, critter_name(a.entry)), (721, Some("rabbit")));
        b.add_critter(None, home, 0).unwrap();
        b.add_critter(None, home, 0).unwrap();
        assert_eq!(b.add_critter(None, home, 0), Err("critter.full"));
        let gs: std::collections::BTreeSet<u64> = b.critters.iter().map(|c| c.guid).collect();
        assert_eq!(gs.len(), 3, "each its own guid");
        assert!(gs.iter().all(|g| g >> 48 == 0xF130 && (*g & 0xFF_FFFF) >= 0xA000));
        let blocks = [(5.0, 5.0, 0.5)];
        let mut steps = 0;
        for ms in (0..120_000).step_by(500) {
            for (_, from, to, t) in b.critter_steps(ms, &blocks) {
                steps += 1;
                assert!(d2(to, home) <= CRITTER_ROAM + 0.9, "{to:?}");
                assert!(d2(to, at(5.0, 5.0)) >= 0.8 - 1e-3, "not inside the prop");
                assert!(t >= 300 && from != to);
            }
        }
        assert!(steps >= 3 * 8, "{steps}");
        assert_eq!(b.critters_away().len(), 3);
        assert!(b.critters.is_empty());
    }

    #[test]
    fn she_looks_around_when_you_stand_still_but_not_too_far() {
        let mut b = Brain::new(11);
        b.arrive(0);
        let p = at(20.0, 0.0);
        let mut wander = None;
        let mut emotes = 0;
        for i in 0..240u64 {
            let ms = i * 1000;
            for a in b.think(&sense(ms, p), &[(21.0, 3.0, 1.0)]) {
                match a {
                    Act::Wander(t) => {
                        wander.get_or_insert((ms, t));
                    }
                    Act::Emote(_) => emotes += 1,
                    _ => {}
                }
            }
        }
        let (ms, t) = wander.expect("a look around");
        assert!(ms >= EXPLORE_AFTER_MS && ms >= STILL_MS);
        assert!(d2(t, at(0.0, 0.0)) <= 22.0 - 1.5 + 1e-3, "inside the Den: {t:?}");
        assert!(d2(t, p) <= EXPLORE_RADIUS + 1.0);
        assert!(emotes >= 1, "a gesture while you stand there");
        // you walking about: no wandering off
        let mut b = Brain::new(11);
        b.arrive(0);
        for i in 0..240u64 {
            let acts = b.think(&sense(i * 1000, at((i % 5) as f32, 0.0)), &[]);
            assert!(acts.iter().all(|a| !matches!(a, Act::Wander(_))));
        }
    }

    #[test]
    fn her_words() {
        assert_eq!(word("quiet"), Some(Word::Talk(Talk::Quiet)));
        assert_eq!(word("Be quiet!"), Some(Word::Talk(Talk::Quiet)));
        assert_eq!(word("talk more"), Some(Word::Talk(Talk::Chatty)));
        assert_eq!(word("you can talk"), Some(Word::Talk(Talk::Normal)));
        assert_eq!(word("help"), Some(Word::Help));
        assert_eq!(word("Help me, please"), Some(Word::Help));
        assert_eq!(word("yes"), Some(Word::Yes));
        assert_eq!(word("No thanks."), Some(Word::No));
        assert_eq!(word("tidy up"), Some(Word::Tidy));
        assert_eq!(word("a pet"), Some(Word::Critter(None)));
        assert_eq!(word("bring a bunny"), Some(Word::Critter(Some(0))));
        assert_eq!(word("a squirrel please"), Some(Word::Critter(Some(1))));
        assert_eq!(word("pets away"), Some(Word::CrittersAway));
        assert_eq!(word("what did I build?"), Some(Word::Memory));
        assert_eq!(word("build a cozy corner"), Some(Word::Build("corner")));
        assert_eq!(word("make me some lights"), Some(Word::Build("lights")));
        assert_eq!(word("build a winter tree"), Some(Word::Build("winter")));
        assert_eq!(word("decorate"), Some(Word::Decorate));
        assert_eq!(word("build a garden"), None, "a stamp: dojo.rs build_words");
        assert_eq!(word("stay"), None, "companion.rs command");
        assert_eq!(word("tips"), None);
        assert_eq!(word("a well by the door"), None, "an ask for the assistant");
        assert_eq!(word("yes and put a bench by the door"), None);
    }

    #[test]
    fn the_gesture_body_and_the_panel_line() {
        let b = emote_body(0xF130_0000_0000_0001, CHEER);
        assert_eq!(b.len(), 12);
        assert_eq!(u32::from_le_bytes(b[..4].try_into().unwrap()), 4);
        assert_eq!(u64::from_le_bytes(b[4..].try_into().unwrap()), 0xF130_0000_0000_0001);
        let mut br = Brain::new(1);
        assert_eq!(br.ui_line(), "talk=normal;doing=follow;mood=calm;offer=-;ask=-;mine=0;critters=0;placed=0;fav=-");
        br.set_talk(Talk::Quiet, 0);
        assert!(br.ui_line().starts_with("talk=quiet;"));
        assert_eq!(br.name_in("Another {fav}?", |_| "x".into()), "Another one of those?");
        br.memory.placed.insert(2413, 2);
        assert_eq!(br.name_in("Another {fav}? Yes.", |e| if e == 2413 { "Wooden Chair".into() } else { "?".into() }), "Another Wooden Chair? Yes.");
        assert_eq!(br.name_in("Plain.", |_| "x".into()), "Plain.");
    }

    #[test]
    fn moods_follow_what_happens() {
        let mut b = Brain::new(1);
        b.arrive(0);
        assert_eq!(b.mood(), Mood::Calm);
        for i in 0..4 {
            b.feed(Event::GuestArrived, i);
        }
        assert_eq!(b.mood(), Mood::Excited);
        // a long still while: back to calm, then sleepy
        let p = at(0.0, 0.0);
        let mut moods = Vec::new();
        for i in 0..3000u64 {
            b.think(&sense(i * 1000, p), &[]);
            moods.push(b.mood());
        }
        assert!(moods.contains(&Mood::Sleepy), "standing still a long time");
        b.feed(Event::Placed { entry: 1, n: 1 }, 3_000_000);
        for i in 0..10u64 {
            b.think(&sense(3_000_000 + i * 1000, at(i as f32, 0.0)), &[]);
        }
        assert_ne!(b.mood(), Mood::Sleepy, "you moving again wakes her");
    }
}
