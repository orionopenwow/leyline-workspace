//! Boss Rush, the light version (docs/BOSS-RUSH.md): a raid's big fight in Den Lite, at the fight's real place on its
//! real map (the player's own game draws the platform from its own files), with our own recreation of its mechanics.
//! It is labelled "light version, not exact" everywhere a player sees it: the exact version is the game server's own
//! boss script on the launcher's Built-in engine (launcher host/bossrush.rs). Starts in seconds, no database.
//!
//! An encounter is plain text, one statement a line (the tale script's style, tale.rs), parsed and checked before it
//! is used: every number in range, every name known, sizes capped, no brand words; nothing in it runs as code.
//!
//! ```text
//! fight frozen-throne
//! title The Frozen Throne
//! map 631
//! start 517.0 -2124.6 840.9 3.14
//! edge 503.6 -2124.7 48 835
//! boss 36597 at 460.0 -2124.0 840.9 face 0 health 9000000 hits 9000 every 2.5 scale 1
//! heroic health 1.4 damage 1.3
//! phase until 70
//!   say "The air turns cold."
//!   every 20 circle "Necrotic plague" r 6 warn 3 hit 12000 at random
//!   every 30 adds 37695 3 at edge
//! phase until 40
//!   say "The edge gives way!"
//!   edge 503.6 -2124.7 38 835
//!   every 15 pulse "Remorseless winter" hit 4000
//!   every 25 soak "Defile" r 5 warn 4 hit 26000 need 3
//! phase until 0
//!   every 12 cone "Soul reaper" angle 60 length 20 warn 2 hit 30000
//! enrage 900
//! ```
//!
//! * `start` is where the raid lands (x y z facing), `edge` the floor that counts (a circle: middle, radius, the
//!   lowest height): off it is a fall. `boss <entry>` is the creature (its model is the player's own game's, by the
//!   open AzerothCore table, creatures.rs), its spot, health for a raid of ten, its swings at its target.
//! * A `phase` lasts while the boss is above `until` per cent; its `say` and `edge` happen as it begins; its abilities
//!   repeat `every` seconds: `circle` (a ring on the ground at someone: leave it before `warn` runs out), `cone` (in
//!   front of the boss), `pulse` (everyone), `soak` (a ring that needs `need` players inside, or everyone takes it),
//!   `adds <entry> <n> at edge|boss` (they go for the healer, then anyone).
//! * Health scales with the raid's size (a tenth of `health` per member, at least three tenths) and `heroic`.
//!
//! The fight itself is [`Fight`]: a small state machine (a seeded random for the same run every time in the tests) fed
//! with the time, where everyone stands and the damage the player did, answering with what to show ([`Ev`]). Companions
//! are simple and readable: the tank holds the boss and faces it away, the healer heals whoever is lowest, the damage
//! dealers hit the boss and step out of rings (most of the time). Std only: `rustc --edition 2021 --test src/bossrush.rs`.

use std::fmt::Write as _;

pub const MAX_BYTES: usize = 8 * 1024;
pub const MAX_PHASES: usize = 6;
pub const MAX_ABILITIES: usize = 8;
pub const MAX_ADDS: usize = 12;
/// The launcher's Boss Rush page writes this into the Dens folder; Den Lite opens the fight it names (once per id).
pub const ASK_FILE: &str = "bossrush.json";
/// Den Lite appends each finished run here (the launcher's board reads it: host/bossrush.rs `run_line`'s format).
pub const RESULTS: &str = "bossrush-results.tsv";
/// The companions' and adds' guids (creature high guid, apart from the Dojo's and the pets').
pub const BOSS_GUID: u64 = 0xF130_0000_0B05_0001;
pub const MEMBER_GUID: u64 = 0xF130_0000_0B05_0100;
pub const ADD_GUID: u64 = 0xF130_0000_0B05_0200;
/// The spell the rings and cones are shown with (a ground-target look the game has; only its look is used).
pub const RING_SPELL: u32 = 2120;
pub const PULSE_SPELL: u32 = 1449;
/// The companions' looks: a human man and woman (the game's own player models).
pub const MEMBER_DISPLAYS: [u32; 2] = [49, 50];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub floor: f32,
}

impl Edge {
    pub fn inside(&self, p: Spot) -> bool {
        ((p.x - self.x).powi(2) + (p.y - self.y).powi(2)).sqrt() <= self.r && p.z >= self.floor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aim {
    Random,
    Tank,
    Healer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddsAt {
    Edge,
    Boss,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Circle { r: f32, aim: Aim },
    Cone { angle: f32, length: f32 },
    Pulse,
    Soak { r: f32, need: u8 },
    Adds { entry: u32, n: u8, at: AddsAt },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ability {
    pub name: String,
    pub every_ms: u64,
    pub warn_ms: u64,
    pub hit: u32,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phase {
    pub until: u8,
    pub say: Option<String>,
    pub edge: Option<Edge>,
    pub abilities: Vec<Ability>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Def {
    pub key: String,
    pub title: String,
    pub map: u32,
    pub start: Spot,
    pub edge: Edge,
    pub boss: u32,
    pub boss_at: Spot,
    pub health: u64,
    pub swing: u32,
    pub swing_ms: u64,
    pub scale: f32,
    pub heroic_health: f32,
    pub heroic_damage: f32,
    pub phases: Vec<Phase>,
    pub enrage_ms: u64,
}

const BANNED: [&str; 3] = ["warcraft", "lich king", "blizzard"];

fn clean(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    !BANNED.iter().any(|w| l.contains(w)) && !l.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow") && !l.contains("://")
}

/// The words of a line, a quoted part kept as one word.
fn words(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                if quoted {
                    out.push(std::mem::take(&mut cur));
                    had = false;
                }
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if had {
                    out.push(std::mem::take(&mut cur));
                    had = false;
                }
            }
            c => {
                cur.push(c);
                had = true;
            }
        }
    }
    if quoted {
        return Err("a quote isn't closed".into());
    }
    if had {
        out.push(cur);
    }
    Ok(out)
}

fn num(w: Option<&String>, what: &str, lo: f32, hi: f32) -> Result<f32, String> {
    let v: f32 = w.ok_or(format!("{what} is missing"))?.parse().map_err(|_| format!("{what} isn't a number"))?;
    if !(lo..=hi).contains(&v) || !v.is_finite() {
        return Err(format!("{what} must be {lo} to {hi}"));
    }
    Ok(v)
}

/// The number after a word (`r 6`, `hit 9000`), when the word is there.
fn after(w: &[String], key: &str, lo: f32, hi: f32) -> Result<Option<f32>, String> {
    match w.iter().position(|x| x == key) {
        Some(i) => num(w.get(i + 1), key, lo, hi).map(Some),
        None => Ok(None),
    }
}

const COORD: f32 = 17_100.0;

fn spot(w: &[String], at: usize, what: &str) -> Result<Spot, String> {
    Ok(Spot {
        x: num(w.get(at), what, -COORD, COORD)?,
        y: num(w.get(at + 1), what, -COORD, COORD)?,
        z: num(w.get(at + 2), what, -3000.0, 3000.0)?,
        o: if w.len() > at + 3 && w[at + 3].parse::<f32>().is_ok() { num(w.get(at + 3), what, -7.0, 7.0)? } else { 0.0 },
    })
}

fn edge(w: &[String], at: usize) -> Result<Edge, String> {
    Ok(Edge {
        x: num(w.get(at), "edge", -COORD, COORD)?,
        y: num(w.get(at + 1), "edge", -COORD, COORD)?,
        r: num(w.get(at + 2), "edge radius", 5.0, 200.0)?,
        floor: num(w.get(at + 3), "edge floor", -3000.0, 3000.0)?,
    })
}

/// An encounter checked; the line and why when it isn't one.
pub fn parse(text: &str) -> Result<Def, String> {
    if text.len() > MAX_BYTES {
        return Err(format!("an encounter is {} KB at most", MAX_BYTES / 1024));
    }
    let mut d = Def {
        key: String::new(),
        title: String::new(),
        map: 0,
        start: Spot { x: 0.0, y: 0.0, z: 0.0, o: 0.0 },
        edge: Edge { x: 0.0, y: 0.0, r: 0.0, floor: 0.0 },
        boss: 0,
        boss_at: Spot { x: 0.0, y: 0.0, z: 0.0, o: 0.0 },
        health: 0,
        swing: 0,
        swing_ms: 2000,
        scale: 1.0,
        heroic_health: 1.0,
        heroic_damage: 1.0,
        phases: Vec::new(),
        enrage_ms: 0,
    };
    let (mut has_start, mut has_edge) = (false, false);
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |e: String| format!("line {}: {e}", n + 1);
        if !clean(line) {
            return Err(at("a word that can't be used".into()));
        }
        let w = words(line).map_err(at)?;
        let head = w[0].as_str();
        let r: Result<(), String> = (|| {
            match head {
                "fight" => {
                    let k = w.get(1).ok_or("the fight's key is missing")?;
                    if k.is_empty() || k.len() > 32 || !k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
                        return Err("a key is lower-case letters, digits and -".into());
                    }
                    d.key = k.clone();
                }
                "title" => {
                    let t = w[1..].join(" ");
                    if t.is_empty() || t.len() > 48 {
                        return Err("a title is 1 to 48 letters".into());
                    }
                    d.title = t;
                }
                "map" => d.map = num(w.get(1), "map", 0.0, 1000.0)? as u32,
                "start" => {
                    d.start = spot(&w, 1, "start")?;
                    has_start = true;
                }
                "edge" => {
                    let e = edge(&w, 1)?;
                    match d.phases.last_mut() {
                        Some(p) => p.edge = Some(e),
                        None => {
                            d.edge = e;
                            has_edge = true;
                        }
                    }
                }
                "boss" => {
                    d.boss = num(w.get(1), "boss entry", 1.0, 100_000.0)? as u32;
                    let i = w.iter().position(|x| x == "at").ok_or("the boss needs `at x y z`")?;
                    d.boss_at = spot(&w, i + 1, "boss")?;
                    d.boss_at.o = after(&w, "face", -7.0, 7.0)?.unwrap_or(0.0);
                    d.health = after(&w, "health", 10_000.0, 100_000_000.0)?.ok_or("the boss needs health")? as u64;
                    d.swing = after(&w, "hits", 0.0, 200_000.0)?.unwrap_or(0.0) as u32;
                    d.swing_ms = (after(&w, "every", 0.5, 10.0)?.unwrap_or(2.0) * 1000.0) as u64;
                    d.scale = after(&w, "scale", 0.2, 5.0)?.unwrap_or(1.0);
                }
                "heroic" => {
                    d.heroic_health = after(&w, "health", 1.0, 3.0)?.unwrap_or(1.0);
                    d.heroic_damage = after(&w, "damage", 1.0, 3.0)?.unwrap_or(1.0);
                }
                "phase" => {
                    if d.phases.len() >= MAX_PHASES {
                        return Err(format!("{MAX_PHASES} phases at most"));
                    }
                    let until = after(&w, "until", 0.0, 99.0)?.ok_or("a phase says `until <per cent>`")? as u8;
                    if d.phases.last().is_some_and(|p| p.until <= until) {
                        return Err("each phase ends lower than the one before".into());
                    }
                    d.phases.push(Phase { until, say: None, edge: None, abilities: Vec::new() });
                }
                "say" => {
                    let p = d.phases.last_mut().ok_or("`say` belongs to a phase")?;
                    let t = w[1..].join(" ");
                    if t.is_empty() || t.len() > 120 {
                        return Err("a line is 1 to 120 letters".into());
                    }
                    p.say = Some(t);
                }
                "every" => {
                    let p = d.phases.last_mut().ok_or("an ability belongs to a phase")?;
                    if p.abilities.len() >= MAX_ABILITIES {
                        return Err(format!("{MAX_ABILITIES} abilities a phase at most"));
                    }
                    let every_ms = (num(w.get(1), "every", 3.0, 300.0)? * 1000.0) as u64;
                    let what = w.get(2).ok_or("what the ability is")?.as_str();
                    let (name, rest) = if what == "adds" { ("Adds".to_string(), &w[3..]) } else { (w.get(3).cloned().ok_or("the ability's name")?, &w[4..]) };
                    if name.is_empty() || name.len() > 40 {
                        return Err("an ability's name is 1 to 40 letters".into());
                    }
                    let warn_ms = (after(rest, "warn", 0.0, 10.0)?.unwrap_or(0.0) * 1000.0) as u64;
                    let hit = after(rest, "hit", 0.0, 500_000.0)?.unwrap_or(0.0) as u32;
                    let kind = match what {
                        "circle" => Kind::Circle {
                            r: after(rest, "r", 1.0, 40.0)?.ok_or("a circle has `r`")?,
                            aim: match rest.iter().position(|x| x == "at").and_then(|i| rest.get(i + 1)).map(String::as_str) {
                                None | Some("random") => Aim::Random,
                                Some("tank") => Aim::Tank,
                                Some("healer") => Aim::Healer,
                                Some(o) => return Err(format!("a circle is at random, tank or healer, not {o}")),
                            },
                        },
                        "cone" => Kind::Cone { angle: after(rest, "angle", 10.0, 180.0)?.unwrap_or(60.0), length: after(rest, "length", 3.0, 80.0)?.unwrap_or(20.0) },
                        "pulse" => Kind::Pulse,
                        "soak" => Kind::Soak { r: after(rest, "r", 1.0, 20.0)?.ok_or("a soak has `r`")?, need: after(rest, "need", 1.0, 10.0)?.unwrap_or(2.0) as u8 },
                        "adds" => {
                            let entry = num(w.get(3), "the adds' entry", 1.0, 100_000.0)? as u32;
                            let n = num(w.get(4), "how many adds", 1.0, MAX_ADDS as f32)? as u8;
                            let at = match w.iter().position(|x| x == "at").and_then(|i| w.get(i + 1)).map(String::as_str) {
                                None | Some("edge") => AddsAt::Edge,
                                Some("boss") => AddsAt::Boss,
                                Some(o) => return Err(format!("adds come at edge or boss, not {o}")),
                            };
                            Kind::Adds { entry, n, at }
                        }
                        o => return Err(format!("no ability called {o}: circle, cone, pulse, soak, adds")),
                    };
                    if warn_ms >= every_ms {
                        return Err("the warning is shorter than the time between".into());
                    }
                    p.abilities.push(Ability { name, every_ms, warn_ms, hit, kind });
                }
                "enrage" => d.enrage_ms = (num(w.get(1), "enrage", 30.0, 3600.0)? * 1000.0) as u64,
                o => return Err(format!("no statement called {o}")),
            }
            Ok(())
        })();
        r.map_err(at)?;
    }
    if d.key.is_empty() || d.title.is_empty() || d.boss == 0 || !has_start || !has_edge {
        return Err("an encounter needs fight, title, map, start, edge and boss".into());
    }
    if d.phases.is_empty() || d.phases.last().map(|p| p.until) != Some(0) {
        return Err("the last phase goes `until 0`".into());
    }
    if !d.edge.inside(d.start) {
        return Err("the raid would land off its own floor".into());
    }
    Ok(d)
}

// ------------------------------------------------------------------------------------------------ the fight

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Tank,
    Healer,
    Damage,
}

/// Someone in the raid: the player (`id` 0) or a companion.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub id: u8,
    pub name: String,
    pub role: Role,
    pub hp: u32,
    pub max: u32,
    pub at: Spot,
    pub bot: bool,
}

impl Member {
    pub fn guid(&self) -> u64 {
        if self.bot {
            MEMBER_GUID + self.id as u64
        } else {
            0
        }
    }
}

/// What the world does for the fight.
#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    /// A line in the chat (the boss's words, the fight's own).
    Say(String),
    /// The middle of the screen.
    Middle(String),
    /// A ring, cone or soak shown at a spot (the cast's look), landing in `ms`.
    Warn { name: String, at: Spot, r: f32, ms: u64 },
    /// Damage to someone (`who` 0: the player): the world applies it to the player's own health.
    Hit { who: u8, amount: u32, what: String },
    /// The player healed by a companion: the world adds it to the player's own health.
    Heal { who: u8, amount: u32 },
    /// A companion's or add's health now.
    Health { guid: u64, cur: u32, max: u32 },
    /// The boss's health now.
    BossHealth { cur: u64, max: u64 },
    Spawn { guid: u64, entry: u32, at: Spot, hp: u32 },
    Gone(u64),
    /// Someone walks (a companion, an add or the boss) to a spot.
    Walk { guid: u64, to: Spot, ms: u32 },
    Phase(u8),
    Edge(Edge),
    Wipe { ms: u64, why: String },
    Clear { ms: u64 },
}

#[derive(Debug, Clone, PartialEq)]
struct Pending {
    name: String,
    at: Spot,
    r: f32,
    hit: u32,
    due: u64,
    kind: Kind,
    facing: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Add {
    pub guid: u64,
    pub entry: u32,
    pub hp: u32,
    pub at: Spot,
    next_hit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Ready,
    Fighting,
    Wiped,
    Cleared,
}

#[derive(Debug, Clone)]
pub struct Fight {
    pub def: Def,
    pub heroic: bool,
    pub members: Vec<Member>,
    pub boss_hp: u64,
    pub boss_max: u64,
    pub boss_at: Spot,
    pub state: State,
    pub phase: usize,
    pub edge: Edge,
    pub adds: Vec<Add>,
    pub started: u64,
    next: Vec<u64>,
    pending: Vec<Pending>,
    next_swing: u64,
    next_bots: u64,
    rng: u64,
    adds_made: u64,
}

/// A raid of `size` (1 to 10): the player and companions in the usual seats (2 tanks, 2 healers, the rest damage;
/// the player's own role takes one of them).
pub fn raid(size: usize, player: Role, player_hp: u32, names: &[&str]) -> Vec<Member> {
    let size = size.clamp(1, 10);
    let mut seats = vec![Role::Tank, Role::Healer, Role::Damage, Role::Damage, Role::Tank, Role::Healer, Role::Damage, Role::Damage, Role::Damage, Role::Damage];
    if let Some(i) = seats.iter().position(|r| *r == player) {
        seats.remove(i);
    } else {
        seats.pop();
    }
    let mut v = vec![Member { id: 0, name: "You".into(), role: player, hp: player_hp, max: player_hp, at: Spot { x: 0.0, y: 0.0, z: 0.0, o: 0.0 }, bot: false }];
    for (i, r) in seats.into_iter().take(size - 1).enumerate() {
        let max = match r {
            Role::Tank => 42_000,
            Role::Healer => 24_000,
            Role::Damage => 26_000,
        };
        let name = names.get(i).copied().unwrap_or("Companion").to_string();
        v.push(Member { id: i as u8 + 1, name, role: r, hp: max, max, at: Spot { x: 0.0, y: 0.0, z: 0.0, o: 0.0 }, bot: true });
    }
    v
}

pub const NAMES: [&str; 9] = ["Bram", "Tessa", "Orrin", "Lyra", "Dain", "Mirel", "Kest", "Sorrel", "Juno"];

fn dist(a: Spot, b: Spot) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn toward(from: Spot, to: Spot, yards: f32) -> Spot {
    let d = dist(from, to).max(0.001);
    let k = (yards / d).min(1.0);
    Spot { x: from.x + (to.x - from.x) * k, y: from.y + (to.y - from.y) * k, z: to.z, o: (to.y - from.y).atan2(to.x - from.x) }
}

impl Fight {
    /// Health for this raid: a tenth of the boss's for each member (three tenths at least), heroic's more.
    pub fn new(def: &Def, heroic: bool, mut members: Vec<Member>, seed: u64) -> Fight {
        let n = members.len().max(3) as f32;
        let max = (def.health as f32 * n / 10.0 * if heroic { def.heroic_health } else { 1.0 }) as u64;
        for (i, m) in members.iter_mut().enumerate() {
            let a = i as f32 * 0.6;
            m.at = Spot { x: def.start.x + a.cos() * (i as f32 * 0.8), y: def.start.y + a.sin() * (i as f32 * 0.8), z: def.start.z, o: def.start.o };
        }
        Fight {
            def: def.clone(),
            heroic,
            members,
            boss_hp: max,
            boss_max: max,
            boss_at: def.boss_at,
            state: State::Ready,
            phase: 0,
            edge: def.edge,
            adds: Vec::new(),
            started: 0,
            next: Vec::new(),
            pending: Vec::new(),
            next_swing: 0,
            next_bots: 0,
            rng: seed.max(1),
            adds_made: 0,
        }
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 32) as u32
    }

    fn dmg(&self, v: u32) -> u32 {
        (v as f32 * if self.heroic { self.def.heroic_damage } else { 1.0 }) as u32
    }

    pub fn player(&self) -> &Member {
        &self.members[0]
    }

    fn alive(&self) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(|m| m.hp > 0)
    }

    fn tank(&self) -> Option<u8> {
        self.alive().find(|m| m.role == Role::Tank).map(|m| m.id).or_else(|| self.alive().next().map(|m| m.id))
    }

    fn pct(&self) -> u8 {
        ((self.boss_hp * 100) / self.boss_max.max(1)) as u8
    }

    /// The pull: the clock starts, the first phase's abilities are timed, the companions take their places.
    pub fn pull(&mut self, now: u64) -> Vec<Ev> {
        if self.state != State::Ready {
            return Vec::new();
        }
        self.state = State::Fighting;
        self.started = now;
        self.next_swing = now + self.def.swing_ms;
        self.next_bots = now;
        let mut out = vec![Ev::Middle(format!("{}: the fight is on!", self.def.title))];
        out.extend(self.enter_phase(0, now));
        out
    }

    fn enter_phase(&mut self, i: usize, now: u64) -> Vec<Ev> {
        self.phase = i;
        let p = self.def.phases[i].clone();
        self.next = p.abilities.iter().map(|a| now + a.every_ms / 2).collect();
        let mut out = vec![Ev::Phase(i as u8 + 1)];
        if let Some(s) = p.say {
            out.push(Ev::Say(s));
        }
        if let Some(e) = p.edge {
            self.edge = e;
            out.push(Ev::Edge(e));
        }
        out
    }

    /// The player's own damage (combat.rs, on the boss's guid).
    pub fn hurt_boss(&mut self, amount: u64, now: u64) -> Vec<Ev> {
        let mut out = Vec::new();
        if self.state == State::Ready {
            out.extend(self.pull(now));
        }
        if self.state != State::Fighting {
            return out;
        }
        self.boss_hp = self.boss_hp.saturating_sub(amount);
        out.push(Ev::BossHealth { cur: self.boss_hp, max: self.boss_max });
        out.extend(self.check_phase(now));
        out
    }

    /// The player's damage on an add.
    pub fn hurt_add(&mut self, guid: u64, amount: u32) -> Vec<Ev> {
        let mut out = Vec::new();
        if let Some(i) = self.adds.iter().position(|a| a.guid == guid) {
            self.adds[i].hp = self.adds[i].hp.saturating_sub(amount);
            if self.adds[i].hp == 0 {
                out.push(Ev::Gone(guid));
                self.adds.remove(i);
            } else {
                out.push(Ev::Health { guid, cur: self.adds[i].hp, max: 30_000 });
            }
        }
        out
    }

    fn check_phase(&mut self, now: u64) -> Vec<Ev> {
        let mut out = Vec::new();
        if self.boss_hp == 0 {
            self.state = State::Cleared;
            let ms = now.saturating_sub(self.started);
            for a in std::mem::take(&mut self.adds) {
                out.push(Ev::Gone(a.guid));
            }
            out.push(Ev::Middle(format!("Cleared in {}!", clock(ms))));
            out.push(Ev::Clear { ms });
            return out;
        }
        while self.phase + 1 < self.def.phases.len() && self.pct() < self.def.phases[self.phase].until.max(1) {
            let next = self.phase + 1;
            out.extend(self.enter_phase(next, now));
        }
        out
    }

    /// The player's own health as the world has it (they heal themselves too); 0 ends it.
    pub fn player_health(&mut self, cur: u32, max: u32) {
        self.members[0].hp = cur;
        self.members[0].max = max.max(1);
    }

    fn wipe(&mut self, now: u64, why: &str) -> Vec<Ev> {
        self.state = State::Wiped;
        let ms = now.saturating_sub(self.started);
        let mut out: Vec<Ev> = std::mem::take(&mut self.adds).into_iter().map(|a| Ev::Gone(a.guid)).collect();
        out.push(Ev::Middle(format!("Wiped: {why}")));
        out.push(Ev::Wipe { ms, why: why.into() });
        out
    }

    fn hit(&mut self, who: u8, amount: u32, what: &str) -> Vec<Ev> {
        let Some(m) = self.members.iter_mut().find(|m| m.id == who) else { return Vec::new() };
        if m.hp == 0 {
            return Vec::new();
        }
        m.hp = m.hp.saturating_sub(amount);
        if m.bot {
            vec![Ev::Health { guid: m.guid(), cur: m.hp, max: m.max }]
        } else {
            vec![Ev::Hit { who, amount, what: what.into() }]
        }
    }

    /// One beat (every 50 ms is fine): where everyone stands (the player's spot; the companions are moved here),
    /// the abilities, the boss's swings, the companions' turns, a fall off the edge, the enrage.
    pub fn tick(&mut self, now: u64, player: Spot) -> Vec<Ev> {
        let mut out = Vec::new();
        self.members[0].at = player;
        if self.state != State::Fighting {
            return out;
        }
        if self.members[0].hp == 0 {
            return self.wipe(now, "you fell in battle");
        }
        if !self.edge.inside(player) {
            out.extend(self.hit(0, u32::MAX, "the fall"));
            return out.into_iter().chain(self.wipe(now, "you fell off the edge")).collect();
        }
        if self.def.enrage_ms > 0 && now >= self.started + self.def.enrage_ms {
            return self.wipe(now, "too slow: the boss went into a rage");
        }
        // what lands now
        let mut i = 0;
        while i < self.pending.len() {
            if now < self.pending[i].due {
                i += 1;
                continue;
            }
            let p = self.pending.remove(i);
            let hit = self.dmg(p.hit);
            let ids: Vec<(u8, Spot)> = self.alive().map(|m| (m.id, m.at)).collect();
            match p.kind {
                Kind::Circle { .. } => {
                    for (id, at) in &ids {
                        if dist(*at, p.at) <= p.r {
                            out.extend(self.hit(*id, hit, &p.name));
                        }
                    }
                }
                Kind::Cone { angle, length } => {
                    let half = angle.to_radians() / 2.0;
                    for (id, at) in &ids {
                        let a = (at.y - p.at.y).atan2(at.x - p.at.x);
                        let mut d = (a - p.facing).abs() % std::f32::consts::TAU;
                        if d > std::f32::consts::PI {
                            d = std::f32::consts::TAU - d;
                        }
                        if dist(*at, p.at) <= length && d <= half {
                            out.extend(self.hit(*id, hit, &p.name));
                        }
                    }
                }
                Kind::Soak { need, .. } => {
                    let inside: Vec<u8> = ids.iter().filter(|(_, at)| dist(*at, p.at) <= p.r).map(|(id, _)| *id).collect();
                    if inside.len() >= need as usize {
                        let share = hit / inside.len().max(1) as u32;
                        for id in inside {
                            out.extend(self.hit(id, share, &p.name));
                        }
                    } else {
                        out.push(Ev::Middle(format!("{}: not enough of you stood in it!", p.name)));
                        for (id, _) in &ids {
                            out.extend(self.hit(*id, hit, &p.name));
                        }
                    }
                }
                Kind::Pulse | Kind::Adds { .. } => {}
            }
        }
        // the phase's abilities
        let abilities = self.def.phases[self.phase].abilities.clone();
        for (k, a) in abilities.iter().enumerate() {
            if now < self.next[k] {
                continue;
            }
            self.next[k] = now + a.every_ms;
            out.extend(self.cast(a, now));
        }
        // the boss swings at its target (the tank)
        if self.def.swing > 0 && now >= self.next_swing {
            self.next_swing = now + self.def.swing_ms;
            if let Some(t) = self.tank() {
                let v = self.dmg(self.def.swing);
                out.extend(self.hit(t, v, "a heavy blow"));
            }
        }
        // adds go for the healer (then anyone) and hit
        for k in 0..self.adds.len() {
            if now < self.adds[k].next_hit {
                continue;
            }
            self.adds[k].next_hit = now + 2000;
            let target = self.alive().find(|m| m.role == Role::Healer).or_else(|| self.alive().next()).map(|m| (m.id, m.at));
            if let Some((id, at)) = target {
                let to = toward(self.adds[k].at, at, 6.0);
                self.adds[k].at = to;
                out.push(Ev::Walk { guid: self.adds[k].guid, to, ms: 1000 });
                if dist(to, at) < 3.0 {
                    let v = self.dmg(1800);
                    out.extend(self.hit(id, v, "an add"));
                }
            }
        }
        // the companions, once a second
        if now >= self.next_bots {
            self.next_bots = now + 1000;
            out.extend(self.bots(now));
        }
        out.extend(self.check_phase(now));
        if self.state == State::Fighting && self.members[0].hp == 0 {
            out.extend(self.wipe(now, "you fell in battle"));
        } else if self.state == State::Fighting && self.alive().count() == 0 {
            out.extend(self.wipe(now, "everyone fell"));
        }
        out
    }

    fn cast(&mut self, a: &Ability, now: u64) -> Vec<Ev> {
        let mut out = Vec::new();
        let alive: Vec<Member> = self.alive().cloned().collect();
        if alive.is_empty() {
            return out;
        }
        match &a.kind {
            Kind::Circle { r, aim } => {
                let who = match aim {
                    Aim::Tank => alive.iter().find(|m| m.role == Role::Tank),
                    Aim::Healer => alive.iter().find(|m| m.role == Role::Healer),
                    Aim::Random => None,
                }
                .cloned()
                .unwrap_or_else(|| alive[self.roll() as usize % alive.len()].clone());
                self.pending.push(Pending { name: a.name.clone(), at: who.at, r: *r, hit: a.hit, due: now + a.warn_ms, kind: a.kind.clone(), facing: 0.0 });
                out.push(Ev::Warn { name: a.name.clone(), at: who.at, r: *r, ms: a.warn_ms });
                if !who.bot {
                    out.push(Ev::Middle(format!("{} on you: move out!", a.name)));
                }
            }
            Kind::Cone { length, .. } => {
                let target = self.tank().and_then(|t| alive.iter().find(|m| m.id == t)).map(|m| m.at).unwrap_or(self.def.start);
                let facing = (target.y - self.boss_at.y).atan2(target.x - self.boss_at.x);
                self.pending.push(Pending { name: a.name.clone(), at: self.boss_at, r: *length, hit: a.hit, due: now + a.warn_ms, kind: a.kind.clone(), facing });
                out.push(Ev::Warn { name: a.name.clone(), at: self.boss_at, r: *length, ms: a.warn_ms });
                out.push(Ev::Middle(format!("{}: get out of the front!", a.name)));
            }
            Kind::Pulse => {
                let v = self.dmg(a.hit);
                for m in &alive {
                    out.extend(self.hit(m.id, v, &a.name));
                }
                out.push(Ev::Say(format!("{} hits everyone.", a.name)));
            }
            Kind::Soak { r, need } => {
                let at = alive[self.roll() as usize % alive.len()].at;
                self.pending.push(Pending { name: a.name.clone(), at, r: *r, hit: a.hit, due: now + a.warn_ms, kind: a.kind.clone(), facing: 0.0 });
                out.push(Ev::Warn { name: a.name.clone(), at, r: *r, ms: a.warn_ms });
                out.push(Ev::Middle(format!("{}: {} of you, stand in it together!", a.name, need)));
            }
            Kind::Adds { entry, n, at } => {
                for k in 0..*n {
                    if self.adds.len() >= MAX_ADDS {
                        break;
                    }
                    let ang = (self.roll() % 628) as f32 / 100.0 + k as f32;
                    let spot = match at {
                        AddsAt::Edge => Spot { x: self.edge.x + ang.cos() * (self.edge.r - 3.0), y: self.edge.y + ang.sin() * (self.edge.r - 3.0), z: self.def.start.z, o: 0.0 },
                        AddsAt::Boss => Spot { x: self.boss_at.x + ang.cos() * 4.0, y: self.boss_at.y + ang.sin() * 4.0, z: self.boss_at.z, o: 0.0 },
                    };
                    self.adds_made += 1;
                    let guid = ADD_GUID + self.adds_made;
                    self.adds.push(Add { guid, entry: *entry, hp: 30_000, at: spot, next_hit: now + 2500 });
                    out.push(Ev::Spawn { guid, entry: *entry, at: spot, hp: 30_000 });
                }
                out.push(Ev::Middle("Adds! Pick them up.".into()));
            }
        }
        out
    }

    /// The companions' turn: the tank holds the boss facing away from the raid, the healer heals the lowest, the
    /// damage dealers hit the boss (or an add going for the healer), and anyone in a ring steps out (most times).
    fn bots(&mut self, now: u64) -> Vec<Ev> {
        let mut out = Vec::new();
        let rings: Vec<(Spot, f32)> = self.pending.iter().filter(|p| matches!(p.kind, Kind::Circle { .. })).map(|p| (p.at, p.r)).collect();
        let soaks: Vec<Spot> = self.pending.iter().filter(|p| matches!(p.kind, Kind::Soak { .. })).map(|p| p.at).collect();
        let boss = self.boss_at;
        let start = self.def.start;
        let mut boss_damage = 0u64;
        for k in 1..self.members.len() {
            if self.members[k].hp == 0 {
                continue;
            }
            let m = self.members[k].clone();
            let mut to = match m.role {
                Role::Tank => toward(boss, start, 4.0),
                Role::Healer => toward(boss, start, 22.0),
                Role::Damage => {
                    let a = k as f32 * 0.9;
                    Spot { x: boss.x + a.cos() * 7.0, y: boss.y + a.sin() * 7.0, z: boss.z, o: 0.0 }
                }
            };
            if let Some(s) = soaks.first().filter(|_| m.role == Role::Damage && k % 2 == 0) {
                to = *s; // half the damage dealers join a soak
            }
            for (c, r) in &rings {
                if dist(to, *c) <= *r && self.roll() % 10 < 8 {
                    to = toward(*c, to, r + 2.0);
                    to = Spot { x: c.x + (to.x - c.x) * 2.0, y: c.y + (to.y - c.y) * 2.0, ..to };
                }
            }
            to.z = self.def.start.z;
            if dist(m.at, to) > 1.0 {
                self.members[k].at = to;
                out.push(Ev::Walk { guid: m.guid(), to, ms: 900 });
            }
            match m.role {
                Role::Healer => {
                    let low = self.members.iter().filter(|x| x.hp > 0 && x.hp < x.max).min_by_key(|x| x.hp * 100 / x.max.max(1)).map(|x| x.id);
                    if let Some(id) = low {
                        let x = self.members.iter_mut().find(|x| x.id == id).expect("there");
                        x.hp = (x.hp + 6000).min(x.max);
                        out.push(if x.bot { Ev::Health { guid: x.guid(), cur: x.hp, max: x.max } } else { Ev::Heal { who: 0, amount: 6000 } });
                    }
                }
                Role::Tank => boss_damage += 1500,
                Role::Damage => {
                    if let Some(a) = self.adds.first().map(|a| a.guid) {
                        out.extend(self.hurt_add(a, 4000));
                    } else {
                        boss_damage += 4500;
                    }
                }
            }
        }
        if boss_damage > 0 {
            self.boss_hp = self.boss_hp.saturating_sub(boss_damage);
            out.push(Ev::BossHealth { cur: self.boss_hp, max: self.boss_max });
        }
        let _ = now;
        out
    }
}

/// The fight in Den Lite now (the world's side, world/bossrush_world.rs), kept in `Shared::rush`.
#[derive(Debug, Clone)]
pub struct Live {
    pub fight: Fight,
    pub class: String,
    pub spec: String,
    /// The boss, the companions and the adds are in the game (the player reached the map).
    pub spawned: bool,
    /// The boss's health as last given to combat.rs (what the player took off since is their damage).
    pub synced: u64,
    /// A phase was skipped to (`.den bossrush phase`): a practice run, not on the board.
    pub practice: bool,
    /// When a finished run goes back to Ready (unix ms; 0: not finished).
    pub reset_at: u64,
    pub seed: u64,
}

impl Live {
    pub fn new(def: &Def, heroic: bool, class: &str, spec: &str, player_hp: u32, role: Role, seed: u64) -> Live {
        Live { fight: Fight::new(def, heroic, raid(10, role, player_hp, &NAMES), seed), class: class.into(), spec: spec.into(), spawned: false, synced: 0, practice: false, reset_at: 0, seed }
    }
    /// The same fight again from the start (Try again).
    pub fn again(&mut self, player_hp: u32) {
        let role = self.fight.members[0].role;
        self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        self.fight = Fight::new(&self.fight.def.clone(), self.fight.heroic, raid(10, role, player_hp, &NAMES), self.seed);
        self.synced = self.fight.boss_hp;
        self.practice = false;
        self.reset_at = 0;
    }
}

/// `Shared::rush`: the fight, and the launcher's last ask (its id, so each opens once).
#[derive(Debug, Default)]
pub struct Rush {
    pub live: Option<Live>,
    /// The fight's units as of the last beat (guid, spot, hostile): what combat sees while a beat is being worked.
    pub cache: Vec<(u64, Spot, bool)>,
    pub next_ask: u64,
    pub asked: String,
    /// `.den bossrush leave` while a beat was being worked: that beat doesn't put the fight back.
    pub left: bool,
}

/// The role of a class and spec word (the launcher's words): tanks and healers by spec, the rest damage.
pub fn role_of(class: &str, spec: &str) -> Role {
    match (class, spec) {
        (_, "prot") | ("deathknight", "blood") | ("druid", "bear") => Role::Tank,
        ("paladin", "holy") | ("priest", "disc") | ("priest", "holy") | ("shaman", "resto") | ("druid", "resto") => Role::Healer,
        _ => Role::Damage,
    }
}

/// A practice jump (`.den bossrush phase <n|end>`): the boss's health at the start of phase n (1-based), or 1 for
/// the killing blow.
pub fn phase_health(f: &Fight, which: &str) -> Option<u64> {
    if which == "end" {
        return Some(1);
    }
    let n: usize = which.parse().ok()?;
    if n == 0 || n > f.def.phases.len() {
        return None;
    }
    let pct = if n == 1 { 100 } else { f.def.phases[n - 2].until as u64 };
    Some((f.boss_max * pct / 100).saturating_sub(1).max(1))
}

/// m:ss.t
pub fn clock(ms: u64) -> String {
    let t = ms / 100;
    format!("{}:{:02}.{}", t / 600, (t / 10) % 60, t % 10)
}

/// A finished run as the launcher's board reads it (launcher host/bossrush.rs `run_line`).
pub fn result_line(key: &str, heroic: bool, class: &str, spec: &str, ms: u64, cleared: bool, at: u64) -> String {
    let w = |s: &str| s.chars().filter(|c| c.is_ascii_lowercase()).take(12).collect::<String>();
    let mut s = String::new();
    let _ = write!(s, "{key}\t{}\tlight\t{}\t{}\t{ms}\t{}\t{at}", if heroic { "heroic" } else { "normal" }, w(class), w(spec), if cleared { "clear" } else { "wipe" });
    s
}

/// The launcher's ask (`<dens>/bossrush.json`: `{"fight", "diff", "class", "spec", "id"}`), read by hand (std only):
/// (fight, heroic, class, spec, id). Anything else in the file is ignored; a fight not in the catalog is refused.
pub fn read_ask(text: &str) -> Option<(String, bool, String, String, String)> {
    if text.len() > 2048 {
        return None;
    }
    let field = |k: &str| -> String {
        let pat = format!("\"{k}\"");
        let Some(i) = text.find(&pat) else { return String::new() };
        let rest = &text[i + pat.len()..];
        let Some(q1) = rest.find('"') else { return String::new() };
        let rest = &rest[q1 + 1..];
        rest.find('"').map(|q2| rest[..q2].to_string()).unwrap_or_default()
    };
    let fight = field("fight");
    let ok = |s: &str| !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !ok(&fight) || encounter(&fight).is_none() {
        return None;
    }
    let (class, spec, id) = (field("class"), field("spec"), field("id"));
    if !ok(&class) || !ok(&spec) || id.is_empty() || id.len() > 40 {
        return None;
    }
    Some((fight, field("diff") == "heroic", class, spec, id))
}

// ------------------------------------------------------------------------------------------------ the catalog

/// The light versions, at each fight's real spot (the same arrival spots as the module's BossfightsLogic.h; the
/// bosses' own spots from the open AzerothCore world data). Numbers are ours: a fight of a few minutes for a hero
/// and nine companions. Not 1:1: docs/BOSS-RUSH.md "The light version".
pub const CATALOG: &[(&str, &str)] = &[
    ("frozen-throne", r#"fight frozen-throne
title The Frozen Throne
map 631
start 517.0 -2124.6 841.4 3.14
edge 503.6 -2124.7 48 835
boss 36597 at 470.0 -2124.4 840.9 face 0 health 6000000 hits 9000 every 2.5 scale 1
heroic health 1.4 damage 1.3
phase until 70
  say "Cold creeps across the platform."
  every 20 circle "Creeping plague" r 6 warn 3 hit 14000 at random
  every 30 adds 37695 3 at edge
phase until 40
  say "The edge gives way! Stay near the middle."
  edge 503.6 -2124.7 36 835
  every 15 pulse "Bitter winter" hit 3500
  every 24 soak "Dark ground" r 5 warn 4 hit 30000 need 3
phase until 0
  say "The blade hungers."
  every 12 cone "Soul harvest" angle 60 length 22 warn 2 hit 30000
  every 18 circle "Creeping plague" r 6 warn 3 hit 14000 at random
enrage 900"#),
    ("bone-lord", r#"fight bone-lord
title The Bone Lord
map 631
start -376.4 2211.1 42.6 3.14
edge -401.4 2211.1 70 30
boss 36612 at -401.4 2211.1 42.1 face 0 health 3500000 hits 8000 every 2 scale 1
heroic health 1.4 damage 1.3
phase until 30
  every 14 circle "Coldflame" r 5 warn 2 hit 9000 at random
  every 30 adds 36619 1 at boss
phase until 0
  say "The bones whirl!"
  every 10 pulse "Bone storm" hit 2500
  every 12 circle "Coldflame" r 5 warn 2 hit 9000 at random
enrage 600"#),
    ("cult-mistress", r#"fight cult-mistress
title The Cult Mistress
map 631
start -609.7 2212.3 52.5 3.18
edge -610.0 2211.4 70 40
boss 36855 at -634.7 2211.4 52.0 face 0 health 3200000 hits 6000 every 2.5 scale 1
heroic health 1.4 damage 1.3
phase until 50
  say "The cult pours in from both doors."
  every 25 adds 37949 2 at edge
  every 16 circle "Death and decay" r 7 warn 3 hit 10000 at random
phase until 0
  say "Her shield breaks: she comes down to fight."
  every 18 circle "Death and decay" r 7 warn 3 hit 10000 at random
  every 20 pulse "Frostbolt volley" hit 3000
enrage 600"#),
    ("twilight-destroyer", r#"fight twilight-destroyer
title The Twilight Destroyer
map 724
start 3131.0 533.4 73.5 0.02
edge 3156.0 533.8 55 60
boss 39863 at 3156.0 533.8 73.0 face 3.16 health 5000000 hits 9000 every 2 scale 1
heroic health 1.5 damage 1.3
phase until 75
  every 18 circle "Fiery combustion" r 6 warn 3 hit 15000 at random
  every 14 cone "Flame breath" angle 70 length 25 warn 2 hit 25000
phase until 50
  say "The dragon slips into the other realm."
  every 16 soak "Soul consumption" r 6 warn 4 hit 28000 need 3
  every 12 pulse "Dark breath" hit 3000
phase until 0
  say "Both realms at once!"
  every 15 circle "Fiery combustion" r 6 warn 3 hit 15000 at random
  every 16 soak "Soul consumption" r 6 warn 4 hit 28000 need 3
enrage 900"#),
    ("obsidian-sanctum", r#"fight obsidian-sanctum
title The Obsidian Sanctum
map 615
start 3245.3 526.3 59.1 1.52
edge 3246.6 551.3 75 40
boss 28860 at 3246.6 551.3 58.6 face 4.66 health 4000000 hits 8000 every 2 scale 1
phase until 0
  every 14 cone "Flame breath" angle 70 length 25 warn 2 hit 22000
  every 22 pulse "Flame tsunami" hit 4000
  every 16 circle "Lava strike" r 5 warn 3 hit 12000 at random
enrage 600"#),
    ("brood-mother", r#"fight brood-mother
title The Brood Mother's Lair
map 249
start -35.5 -216.7 -87.2 6.18
edge -10.6 -219.4 80 -110
boss 10184 at -10.6 -219.4 -87.7 face 3.04 health 3500000 hits 7000 every 2 scale 1
phase until 65
  every 12 cone "Flame breath" angle 60 length 22 warn 2 hit 18000
phase until 40
  say "She takes to the air! Whelps pour out."
  every 20 adds 11262 4 at edge
  every 10 circle "Fireball" r 4 warn 2 hit 9000 at random
phase until 0
  say "She lands, furious."
  every 25 pulse "Bellowing roar" hit 3000
  every 12 cone "Flame breath" angle 60 length 22 warn 2 hit 18000
enrage 600"#),
    ("stone-watcher", r#"fight stone-watcher
title The Stone Watcher
map 624
start 113.9 -101.1 91.9 6.27
edge 138.9 -101.5 55 70
boss 31125 at 138.9 -101.5 91.4 face 3.12 health 2500000 hits 8000 every 2 scale 1
phase until 0
  every 15 circle "Choking cloud" r 6 warn 3 hit 9000 at random
  every 20 pulse "Stomp" hit 3000
enrage 420"#),
];

/// A catalog fight's encounter, parsed (the catalog's own text always parses: a test says so).
pub fn encounter(key: &str) -> Option<Def> {
    CATALOG.iter().find(|(k, _)| *k == key).and_then(|(_, t)| parse(t).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_catalog_parses_and_lands_on_its_floor() {
        for (k, t) in CATALOG {
            let d = parse(t).unwrap_or_else(|e| panic!("{k}: {e}"));
            assert_eq!(d.key, *k);
            assert!(d.edge.inside(d.start), "{k}");
            assert!(d.edge.inside(Spot { z: d.edge.floor + 1.0, ..d.boss_at }) || k == &"frozen-throne", "{k}: the boss stands on the floor");
            assert!(clean(&d.title));
            assert!(d.enrage_ms >= 300_000);
        }
        assert_eq!(CATALOG.len(), 7);
        assert!(encounter("spell-weaver").is_none(), "no light version of the Eye yet");
    }

    #[test]
    fn bad_encounters_say_where_and_why() {
        let good = CATALOG[6].1;
        assert!(parse(&good.replace("boss 31125", "boss 0")).unwrap_err().contains("boss entry"));
        assert!(parse(&good.replace("every 15 circle", "every 15 laser")).unwrap_err().contains("no ability called laser"));
        assert!(parse(&format!("{good}\nrun rm -rf /")).unwrap_err().contains("no statement called run"));
        assert!(parse(&good.replace("The Stone Watcher", "The Lich King")).unwrap_err().contains("can't be used"));
        assert!(parse(&good.replace("phase until 0", "phase until 10")).unwrap_err().contains("until 0"));
        assert!(parse(&good.replace("r 6 warn 3", "r 6 warn 30")).is_err(), "a warning longer than its timer");
        assert!(parse(&good.replace("start 113.9", "start 900.0")).unwrap_err().contains("off its own floor"));
        assert!(parse(&"# x\n".repeat(MAX_BYTES)).unwrap_err().contains("KB at most"));
        assert!(parse("title \"oops").is_err());
        let many = format!("{good}\n{}", "phase until 0\n".repeat(7));
        assert!(parse(&many).is_err());
    }

    fn fight(key: &str, seed: u64) -> Fight {
        let d = encounter(key).unwrap();
        Fight::new(&d, false, raid(10, Role::Damage, 30_000, &NAMES), seed)
    }

    #[test]
    fn health_scales_with_the_raid_and_heroic() {
        let d = encounter("stone-watcher").unwrap();
        let ten = Fight::new(&d, false, raid(10, Role::Damage, 30_000, &NAMES), 1).boss_max;
        let five = Fight::new(&d, false, raid(5, Role::Damage, 30_000, &NAMES), 1).boss_max;
        let one = Fight::new(&d, false, raid(1, Role::Damage, 30_000, &NAMES), 1).boss_max;
        let heroic = Fight::new(&d, true, raid(10, Role::Damage, 30_000, &NAMES), 1).boss_max;
        assert_eq!(ten, 2_500_000);
        assert_eq!(five, 1_250_000);
        assert_eq!(one, 750_000, "three tenths at least");
        assert_eq!(heroic, 2_500_000, "this boss has no heroic numbers");
        let ft = encounter("frozen-throne").unwrap();
        assert_eq!(Fight::new(&ft, true, raid(10, Role::Damage, 30_000, &NAMES), 1).boss_max, 8_400_000);
        let r = raid(10, Role::Tank, 30_000, &NAMES);
        assert_eq!(r.iter().filter(|m| m.role == Role::Tank).count(), 2, "the player is one of the two tanks");
        assert_eq!(r.iter().filter(|m| m.role == Role::Healer).count(), 2);
    }

    #[test]
    fn pull_phases_the_edge_and_a_clear() {
        let mut f = fight("frozen-throne", 7);
        let at = f.def.start;
        assert!(f.tick(0, at).is_empty(), "nothing before the pull");
        let ev = f.hurt_boss(1000, 1000);
        assert!(ev.contains(&Ev::Phase(1)) && f.state == State::Fighting, "the first hit pulls");
        // to 69%: phase two, the edge moves in
        let ev = f.hurt_boss(f.boss_max * 31 / 100, 2000);
        assert!(ev.contains(&Ev::Phase(2)), "{ev:?}");
        assert!(ev.iter().any(|e| matches!(e, Ev::Edge(e) if e.r == 36.0)));
        assert_eq!(f.edge.r, 36.0);
        let ev = f.hurt_boss(f.boss_hp, 3000);
        assert!(ev.iter().any(|e| matches!(e, Ev::Clear { ms: 2000 })), "{ev:?}");
        assert_eq!(f.state, State::Cleared);
    }

    #[test]
    fn rings_warn_then_hit_whoever_stays() {
        let mut f = fight("stone-watcher", 3);
        let at = f.def.start;
        f.hurt_boss(1, 0);
        let mut warned = None;
        for t in (0..20_000).step_by(50) {
            for e in f.tick(t, at) {
                if let Ev::Warn { name, at: w, r, ms } = &e {
                    if name == "Choking cloud" && warned.is_none() {
                        warned = Some((t, *w, *r, *ms));
                    }
                }
            }
            if warned.is_some() {
                break;
            }
        }
        let (t0, ring, r, ms) = warned.expect("a ring by 20 s");
        assert_eq!((r, ms), (6.0, 3000));
        // stand in it (the player), then it lands: the player is hit
        let before: Vec<Ev> = (0..=((ms + 100) / 50)).flat_map(|k| f.tick(t0 + k * 50, ring)).collect();
        assert!(before.iter().any(|e| matches!(e, Ev::Hit { who: 0, amount: 9000, .. })), "standing in it hurts");
    }

    #[test]
    fn soaks_need_enough_people_and_cones_hit_the_front() {
        let d = parse(&CATALOG[0].1.replace("every 24 soak \"Dark ground\" r 5 warn 4 hit 30000 need 3", "every 24 soak \"Dark ground\" r 5 warn 4 hit 30000 need 9")).unwrap();
        let mut f = Fight::new(&d, false, raid(10, Role::Damage, 30_000, &NAMES), 11);
        f.hurt_boss(f.boss_max * 35 / 100, 0);
        assert_eq!(f.phase, 1);
        let mut said = false;
        for t in (0..60_000).step_by(50) {
            if f.tick(t, f.def.start).iter().any(|e| matches!(e, Ev::Middle(m) if m.contains("not enough of you"))) {
                said = true;
                break;
            }
            if f.state != State::Fighting {
                break;
            }
        }
        assert!(said, "nine needed, too few in it: everyone takes it");
        // the cone: in front of the boss toward the tank
        let mut f = fight("obsidian-sanctum", 5);
        f.hurt_boss(1, 0);
        let tank = f.members.iter().find(|m| m.role == Role::Tank).unwrap().id;
        f.members.iter_mut().find(|m| m.id == tank).unwrap().at = Spot { x: f.boss_at.x, y: f.boss_at.y - 5.0, ..f.boss_at };
        let ab = f.def.phases[0].abilities[0].clone();
        let _ = f.cast(&ab, 100);
        let front = Spot { x: f.boss_at.x, y: f.boss_at.y - 10.0, z: f.boss_at.z, o: 0.0 };
        let ev: Vec<Ev> = (0..100).flat_map(|k| f.tick(100 + k * 50, front)).collect();
        assert!(ev.iter().any(|e| matches!(e, Ev::Hit { who: 0, amount: 22000, .. })), "in front: hit");
    }

    #[test]
    fn wipes_off_the_edge_at_zero_health_and_at_the_enrage() {
        let mut f = fight("frozen-throne", 1);
        f.hurt_boss(1, 0);
        let off = Spot { x: f.edge.x + 60.0, ..f.def.start };
        let ev = f.tick(500, off);
        assert!(ev.iter().any(|e| matches!(e, Ev::Wipe { why, .. } if why.contains("edge"))), "{ev:?}");
        let mut f = fight("frozen-throne", 1);
        f.hurt_boss(1, 0);
        f.player_health(0, 30_000);
        assert!(f.tick(100, f.def.start).iter().any(|e| matches!(e, Ev::Wipe { .. })));
        let mut f = fight("stone-watcher", 1);
        f.hurt_boss(1, 0);
        f.boss_hp = u64::MAX / 2;
        f.boss_max = u64::MAX / 2;
        let ev = f.tick(f.def.enrage_ms + 1, f.def.start);
        assert!(ev.iter().any(|e| matches!(e, Ev::Wipe { why, .. } if why.contains("rage"))));
    }

    #[test]
    fn the_companions_alone_bring_a_short_fight_down_the_same_way_every_time() {
        let run = |seed: u64| {
            let mut f = fight("stone-watcher", seed);
            f.hurt_boss(1, 0);
            let at = f.def.start;
            let mut t = 0;
            while f.state == State::Fighting && t < 1_000_000 {
                t += 50;
                // the player heals back between hits (as their own spells would)
                let (cur, max) = (f.player().hp.max(1), f.player().max);
                let evs = f.tick(t, at);
                let lost: u32 = evs.iter().filter_map(|e| if let Ev::Hit { who: 0, amount, .. } = e { Some(*amount) } else { None }).sum();
                let healed: u32 = evs.iter().filter_map(|e| if let Ev::Heal { who: 0, amount } = e { Some(*amount) } else { None }).sum();
                f.player_health((cur + healed).saturating_sub(lost).min(max), max);
            }
            (f.state, t, f.members.iter().filter(|m| m.hp == 0).map(|m| format!("{:?}", m.role)).collect::<Vec<_>>())
        };
        let (a, b) = (run(42), run(42));
        assert_eq!(a, b, "the same seed, the same fight");
        assert_eq!(a.0, State::Cleared, "nine companions bring it down: {a:?}");
        assert!(a.2.is_empty(), "nobody fell: {:?}", a.2);
        assert!(a.1 > 60_000 && a.1 < 420_000, "a fight of minutes, not seconds: {} ms", a.1);
    }

    #[test]
    fn adds_come_and_go() {
        let mut f = fight("brood-mother", 9);
        f.hurt_boss(f.boss_max * 40 / 100, 0);
        assert_eq!(f.phase, 1);
        let mut spawned = Vec::new();
        for t in (0..15_000).step_by(50) {
            for e in f.tick(t, f.def.start) {
                if let Ev::Spawn { guid, entry, .. } = e {
                    spawned.push((guid, entry));
                }
            }
        }
        assert!(spawned.len() >= 4 && spawned.iter().all(|s| s.1 == 11262), "{spawned:?}");
        let g = f.adds.first().map(|a| a.guid);
        if let Some(g) = g {
            assert!(f.hurt_add(g, 1_000_000).contains(&Ev::Gone(g)));
        }
        let ev = f.hurt_boss(f.boss_hp, 20_000);
        assert!(ev.iter().any(|e| matches!(e, Ev::Clear { .. })));
        assert!(f.adds.is_empty(), "a clear takes the adds away");
    }

    #[test]
    fn roles_practice_jumps_and_try_again() {
        assert_eq!((role_of("warrior", "prot"), role_of("deathknight", "blood"), role_of("priest", "disc"), role_of("mage", "fire")), (Role::Tank, Role::Tank, Role::Healer, Role::Damage));
        let d = encounter("frozen-throne").unwrap();
        let mut l = Live::new(&d, false, "mage", "fire", 30_000, Role::Damage, 5);
        assert_eq!(phase_health(&l.fight, "1"), Some(l.fight.boss_max - 1));
        assert_eq!(phase_health(&l.fight, "2"), Some(l.fight.boss_max * 70 / 100 - 1));
        assert_eq!(phase_health(&l.fight, "end"), Some(1));
        assert_eq!(phase_health(&l.fight, "9"), None);
        l.fight.hurt_boss(10, 0);
        l.again(30_000);
        assert_eq!(l.fight.state, State::Ready);
        assert_eq!(l.fight.boss_hp, l.fight.boss_max);
    }

    #[test]
    fn the_launchers_ask_and_the_result_line() {
        let a = read_ask(r#"{"fight": "bone-lord", "diff": "heroic", "class": "warrior", "spec": "prot", "id": "1762400000"}"#).unwrap();
        assert_eq!(a, ("bone-lord".into(), true, "warrior".into(), "prot".into(), "1762400000".into()));
        assert!(read_ask(r#"{"fight": "spell-weaver", "class": "mage", "spec": "fire", "id": "1"}"#).is_none(), "no light version");
        assert!(read_ask(r#"{"fight": "../x", "class": "mage", "spec": "fire", "id": "1"}"#).is_none());
        assert!(read_ask("{}").is_none());
        assert_eq!(result_line("bone-lord", true, "warrior", "prot", 61_000, true, 9), "bone-lord\theroic\tlight\twarrior\tprot\t61000\tclear\t9");
        assert_eq!(clock(61_000), "1:01.0");
    }
}
