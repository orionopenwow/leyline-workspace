//! Duel tournaments in the Den (owner 4 Nov: "duel tournaments in the Den ... the host makes a bracket, guests sign
//! up in-game ... matches called in order with a countdown, wins recorded ... spectators watch"; the ideas list and
//! the guide are docs/DEN-TOURNAMENTS.md).
//!
//! * The host makes one: `.den tourney new single|robin|king [bo1|bo3|bo5|bo7] [a title]` (single elimination, round
//!   robin, king of the hill; best of N rounds a match). The Leyline UI's Tournament window (WrathUnleashedUI
//!   DenTourney.lua, from the Den bar) sends the same commands.
//! * Everyone signs up: `.den tourney join` (and `leave`). The host can add someone by name (`add <name>`) or take
//!   them out (`remove <name>`).
//! * The arena: `.den tourney ring [yards]` marks a ring where the host stands (6 to 30 yards across; the Dojo's
//!   `.den dojo ring` builds a fenced one to see it by). With a ring, leaving it during a fight loses the round
//!   ("ring out"), and fighters must be in it for the countdown.
//! * `.den tourney start`: the matches are called one at a time, in order: "Next: Rowan vs Briar", both to the ring,
//!   a 3-2-1 countdown, "Fight!". A round ends when a fighter yields (`.den tourney yield`, or /yield's word "I
//!   yield"), steps out of the ring, or the host calls it (`.den tourney win <name>`). First to the rounds the match
//!   needs wins it; the bracket moves on by itself.
//! * Den Lite has no player-against-player combat: nothing takes health off another player here, so the fight is
//!   played out (emotes, the sandbox's spells look the part) and its end is said by the players, the ring, or the
//!   host. On the full Den the realm's own duels decide it (ov_duels.lua); the tournament there is still to do.
//! * Spectators are everyone else in the Den: they see the fight, hear every call, and are asked once to step out of
//!   the ring.
//! * When it ends the champion is announced, and the results go in the Den file (`tournaments`, the last 10: kept
//!   with the Den's autosave and any `.den save`), which the launcher's Saved Dens card shows ("Champion: Rowan").
//!
//! Everything but the glue at the bottom is plain state, tested below.

use serde_json::{json, Value};
use std::sync::Mutex;

/// Before a match is called (after the last one ended), so the room can read the result.
pub const PAUSE_MS: u64 = 4000;
/// The countdown.
pub const COUNT_MS: u64 = 3000;
/// A called fighter not in the Den (or not in the ring) this long loses the match (a no-show).
pub const NO_SHOW_MS: u64 = 60_000;
/// Out of the ring this long during a fight: a ring out (a step over the line and back isn't one).
pub const RING_OUT_MS: u64 = 1500;
/// The ring's line is this much past its radius (a fighter at the edge isn't out).
pub const RING_SLACK: f32 = 1.5;
/// A round this long: the host is asked to call it (once).
pub const LONG_ROUND_MS: u64 = 180_000;
pub const MAX_PLAYERS: usize = 32;
/// Round robin's matches grow fast: everyone plays everyone.
pub const MAX_ROBIN: usize = 10;
/// King of the hill: wins in a row that crown the king.
pub const CROWN: u32 = 3;
/// Results kept in the Den file.
pub const KEEP_RESULTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Single,
    Robin,
    King,
}

impl Format {
    pub fn from_word(w: &str) -> Option<Format> {
        match w {
            "single" | "bracket" | "elimination" | "se" => Some(Format::Single),
            "robin" | "roundrobin" | "rr" | "league" => Some(Format::Robin),
            "king" | "koth" | "hill" => Some(Format::King),
            _ => None,
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Format::Single => "single",
            Format::Robin => "robin",
            Format::King => "king",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Format::Single => "Single elimination",
            Format::Robin => "Round robin",
            Format::King => "King of the hill",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub round: u32,
    pub a: String,
    /// "" is a bye: `a` goes through.
    pub b: String,
    pub wa: u8,
    pub wb: u8,
    pub winner: Option<String>,
    /// How it ended: "won", "bye", "no-show", "left".
    pub how: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Signup,
    /// The next match is called at this time.
    Waiting(u64),
    /// Called: the two fighters to the ring (since).
    Called(u64),
    /// The countdown: the fight starts at the first; the last number said.
    Countdown(u64, u8),
    /// Fighting (since).
    Fight(u64),
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ring {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

impl Ring {
    pub fn inside(&self, x: f32, y: f32) -> bool {
        ((x - self.x).powi(2) + (y - self.y).powi(2)).sqrt() <= self.r + RING_SLACK
    }
}

/// What happened, for the glue to say and send.
#[derive(Debug, Clone, PartialEq)]
pub enum Ev {
    /// A line to everyone in the Den.
    All(String),
    /// The Tournament windows are sent the state again.
    Refresh,
    /// The tournament ended: its results go in the Den file.
    Finished,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tourney {
    pub title: String,
    pub format: Format,
    pub best_of: u8,
    /// A house rule from the ideas list (docs/DEN-TOURNAMENTS.md), said with each call; "" for none.
    pub twist: String,
    pub host: String,
    pub players: Vec<String>,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    pub phase: Phase,
    pub ring: Option<Ring>,
    /// King of the hill: the waiting line, the king's wins in a row.
    pub queue: Vec<String>,
    pub streak: u32,
    pub champion: Option<String>,
    /// When each fighter of the current match was first seen out of the ring (or out of the Den).
    out: [Option<u64>; 2],
    warned: bool,
    long_said: bool,
}

/// The twists the host can name (`.den tourney twist <word>`): shown and said; the ring ones are kept by the ring.
pub const TWISTS: &[(&str, &str)] = &[
    ("ringout", "Ring out: leave the ring and you lose the round"),
    ("classlock", "Class-locked: everyone plays the same class"),
    ("normalised", "Gear-normalised: everyone in the same set"),
    ("nopotions", "No potions, no healing spells"),
    ("melee", "Melee only"),
    ("handicap", "Handicap: the higher seed fights at half speed"),
    ("mirror", "Mirror match: both pick the same class and spec"),
    ("sudden", "Sudden death: the final is one round"),
    ("mounted", "Mounted joust: fight from your mount"),
    ("costume", "Costume rules: dress the part or forfeit"),
];

pub fn clean_name(s: &str) -> Option<String> {
    let s = s.trim();
    if !(2..=12).contains(&s.len()) || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut c = s.chars();
    let f = c.next()?;
    Some(f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase())
}

fn needed(best_of: u8) -> u8 {
    best_of / 2 + 1
}

impl Tourney {
    pub fn new(title: &str, format: Format, best_of: u8, host: &str) -> Tourney {
        let best_of = match best_of {
            1 | 3 | 5 | 7 => best_of,
            _ => 1,
        };
        let title: String = title.chars().filter(|c| c.is_ascii_alphanumeric() || " '-".contains(*c)).take(32).collect();
        let title = if title.trim().is_empty() { format!("{}'s {}", host, format.title()) } else { title.trim().to_string() };
        Tourney {
            title,
            format,
            best_of,
            twist: String::new(),
            host: host.to_string(),
            players: Vec::new(),
            matches: Vec::new(),
            current: None,
            phase: Phase::Signup,
            ring: None,
            queue: Vec::new(),
            streak: 0,
            champion: None,
            out: [None, None],
            warned: false,
            long_said: false,
        }
    }

    fn cap(&self) -> usize {
        if self.format == Format::Robin { MAX_ROBIN } else { MAX_PLAYERS }
    }

    pub fn join(&mut self, name: &str) -> Result<String, String> {
        let n = clean_name(name).ok_or("That isn't a character's name.")?;
        if self.phase != Phase::Signup {
            return Err("Sign-ups are closed: it has started. Watch from outside the ring!".into());
        }
        if self.players.iter().any(|p| p.eq_ignore_ascii_case(&n)) {
            return Err(format!("{n} is already signed up."));
        }
        if self.players.len() >= self.cap() {
            return Err(format!("{} is full ({} players).", self.title, self.cap()));
        }
        self.players.push(n.clone());
        Ok(format!("{n} signed up for {} ({} in).", self.title, self.players.len()))
    }

    /// Out of the tournament: before it starts, off the list; once it runs, their matches still to play go to the
    /// other fighter ("left").
    pub fn leave(&mut self, name: &str, now: u64) -> Vec<Ev> {
        let Some(n) = self.players.iter().find(|p| p.eq_ignore_ascii_case(name)).cloned() else { return Vec::new() };
        if self.phase == Phase::Signup {
            self.players.retain(|p| p != &n);
            return vec![Ev::All(format!("{n} left the sign-ups ({} in).", self.players.len())), Ev::Refresh];
        }
        if self.phase == Phase::Done {
            return Vec::new();
        }
        let mut ev = vec![Ev::All(format!("{n} left the tournament."))];
        self.queue.retain(|p| p != &n);
        let live = self.current;
        let open: Vec<usize> = (0..self.matches.len()).filter(|i| self.matches[*i].winner.is_none() && (self.matches[*i].a == n || self.matches[*i].b == n)).collect();
        for i in open {
            let m = &mut self.matches[i];
            let other = if m.a == n { m.b.clone() } else { m.a.clone() };
            m.winner = Some(other);
            m.how = "left".into();
            if live == Some(i) {
                self.current = None;
            }
        }
        if live.is_some() && self.current.is_none() {
            ev.extend(self.after_match(now));
        }
        ev.push(Ev::Refresh);
        ev
    }

    pub fn start(&mut self, now: u64) -> Result<Vec<Ev>, String> {
        if self.phase != Phase::Signup {
            return Err("It has already started.".into());
        }
        if self.players.len() < 2 {
            return Err("At least two players first (.den tourney join).".into());
        }
        match self.format {
            Format::Single => self.matches = single_round(&self.players, 1),
            Format::Robin => self.matches = robin(&self.players),
            Format::King => {
                self.queue = self.players[1..].to_vec();
                let king = self.players[0].clone();
                let first = self.queue.remove(0);
                self.matches = vec![Match { round: 1, a: king, b: first, wa: 0, wb: 0, winner: None, how: String::new() }];
            }
        }
        self.phase = Phase::Waiting(now + PAUSE_MS);
        let mut ev = vec![Ev::All(format!(
            "{} begins: {}, best of {}, {} players.{}",
            self.title,
            self.format.title(),
            self.best_of,
            self.players.len(),
            if self.ring.is_some() { " Fights are in the ring: step out and you lose the round." } else { "" }
        ))];
        for m in self.matches.iter().filter(|m| m.how == "bye") {
            ev.push(Ev::All(format!("{} goes through to the next round (a bye).", m.a)));
        }
        ev.push(Ev::Refresh);
        Ok(ev)
    }

    /// The next match to call: the first one not played.
    pub fn next_match(&self) -> Option<usize> {
        self.matches.iter().position(|m| m.winner.is_none())
    }

    /// The fighters of the match being played.
    pub fn fighters(&self) -> Option<(String, String)> {
        self.current.map(|i| (self.matches[i].a.clone(), self.matches[i].b.clone()))
    }

    /// One round to `name` (a fighter of the current match, while the fight is on, or before it when the host calls
    /// it). `how`: what is said ("yielded", "ring out", "the host's call").
    pub fn round_to(&mut self, name: &str, how: &str, now: u64) -> Result<Vec<Ev>, String> {
        let i = self.current.ok_or("No match is being fought now.")?;
        if !matches!(self.phase, Phase::Fight(_) | Phase::Countdown(_, _) | Phase::Called(_)) {
            return Err("No match is being fought now.".into());
        }
        let need = if self.final_round() && self.twist == "sudden" { 1 } else { needed(self.best_of) };
        let m = &mut self.matches[i];
        let a_won = if m.a.eq_ignore_ascii_case(name) {
            true
        } else if m.b.eq_ignore_ascii_case(name) {
            false
        } else {
            return Err(format!("{name} isn't fighting now ({} vs {}).", m.a, m.b));
        };
        if a_won { m.wa += 1 } else { m.wb += 1 }
        let (w, l) = if a_won { (m.a.clone(), m.b.clone()) } else { (m.b.clone(), m.a.clone()) };
        let score = format!("{}-{}", m.wa.max(m.wb), m.wa.min(m.wb));
        self.out = [None, None];
        self.long_said = false;
        if m.wa >= need || m.wb >= need {
            m.winner = Some(w.clone());
            m.how = "won".into();
            let mut ev = vec![Ev::All(format!("{w} wins the match against {l}, {score} ({how})."))];
            self.current = None;
            ev.extend(self.after_match(now));
            ev.push(Ev::Refresh);
            return Ok(ev);
        }
        self.phase = Phase::Countdown(now + COUNT_MS, 0);
        Ok(vec![Ev::All(format!("Round to {w} ({how}). {score}. Next round: back to your places...")), Ev::Refresh])
    }

    /// The other fighter takes the round.
    pub fn round_lost(&mut self, name: &str, how: &str, now: u64) -> Result<Vec<Ev>, String> {
        let (a, b) = self.fighters().ok_or("No match is being fought now.")?;
        let other = if a.eq_ignore_ascii_case(name) { b } else if b.eq_ignore_ascii_case(name) { a } else { return Err("You aren't fighting now.".into()) };
        self.round_to(&other, how, now)
    }

    fn final_round(&self) -> bool {
        match (self.format, self.current) {
            (Format::Single, Some(i)) => self.matches.iter().filter(|m| m.round == self.matches[i].round).count() == 1 && self.players.len() > 1,
            _ => false,
        }
    }

    /// After a match: the next one waits, a new round is drawn, or the champion is crowned.
    fn after_match(&mut self, now: u64) -> Vec<Ev> {
        let mut ev = Vec::new();
        match self.format {
            Format::Single => {
                let last = self.matches.iter().map(|m| m.round).max().unwrap_or(1);
                let this: Vec<&Match> = self.matches.iter().filter(|m| m.round == last).collect();
                if this.iter().all(|m| m.winner.is_some()) {
                    let winners: Vec<String> = this.iter().filter_map(|m| m.winner.clone()).collect();
                    if winners.len() <= 1 {
                        return self.finish(winners.first().cloned(), now);
                    }
                    let next = single_round(&winners, last + 1);
                    ev.push(Ev::All(format!("Round {} is drawn: {}.", last + 1, next.iter().map(|m| if m.b.is_empty() { format!("{} (a bye)", m.a) } else { format!("{} vs {}", m.a, m.b) }).collect::<Vec<_>>().join(", "))));
                    self.matches.extend(next);
                }
            }
            Format::Robin => {
                if self.next_match().is_none() {
                    let top = self.standings().first().map(|s| s.0.clone());
                    return self.finish(top, now);
                }
            }
            Format::King => {
                let Some(m) = self.matches.last().cloned() else { return self.finish(None, now) };
                let king = m.winner.clone().unwrap_or(m.a.clone());
                let loser = if m.a == king { m.b.clone() } else { m.a.clone() };
                self.streak = if m.a == king { self.streak + 1 } else { 1 };
                if !loser.is_empty() && self.players.contains(&loser) && m.how != "left" {
                    self.queue.push(loser);
                }
                let played = self.matches.len();
                if self.streak >= CROWN || self.queue.is_empty() || played >= self.players.len() * 3 {
                    return self.finish(Some(king), now);
                }
                if self.streak > 1 {
                    ev.push(Ev::All(format!("{king} holds the hill: {} in a row ({CROWN} wins the crown).", self.streak)));
                } else {
                    ev.push(Ev::All(format!("{king} takes the hill!")));
                }
                let next = self.queue.remove(0);
                self.matches.push(Match { round: played as u32 + 1, a: king, b: next, wa: 0, wb: 0, winner: None, how: String::new() });
            }
        }
        self.phase = Phase::Waiting(now + PAUSE_MS);
        ev
    }

    fn finish(&mut self, champ: Option<String>, _now: u64) -> Vec<Ev> {
        self.phase = Phase::Done;
        self.current = None;
        self.champion = champ.clone();
        let line = match &champ {
            Some(c) => format!("{c} is the champion of {}! Well fought, everyone.", self.title),
            None => format!("{} is over.", self.title),
        };
        vec![Ev::All(line), Ev::Finished, Ev::Refresh]
    }

    /// Wins and losses per player (round robin's table; any format's tally), best first: match wins, then fewer
    /// match losses, then rounds won, then the signup order.
    pub fn standings(&self) -> Vec<(String, u32, u32, u32)> {
        let mut t: Vec<(String, u32, u32, u32)> = self.players.iter().map(|p| (p.clone(), 0, 0, 0)).collect();
        for m in &self.matches {
            for row in t.iter_mut() {
                if row.0 == m.a {
                    row.3 += m.wa as u32;
                } else if row.0 == m.b {
                    row.3 += m.wb as u32;
                } else {
                    continue;
                }
                match &m.winner {
                    Some(_) if m.b.is_empty() => {}
                    Some(w) if *w == row.0 => row.1 += 1,
                    Some(_) => row.2 += 1,
                    None => {}
                }
            }
        }
        let order = |n: &str| self.players.iter().position(|p| p == n).unwrap_or(usize::MAX);
        t.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(b.3.cmp(&a.3)).then(order(&a.0).cmp(&order(&b.0))));
        t
    }

    /// The ticker: called matches, the countdown, ring outs, no-shows. `here`: everyone in the Den now (name, x, y).
    pub fn tick(&mut self, now: u64, here: &[(String, f32, f32)]) -> Vec<Ev> {
        let at = |n: &str| here.iter().find(|h| h.0.eq_ignore_ascii_case(n)).map(|h| (h.1, h.2));
        let in_ring = |n: &str, ring: Option<Ring>| match (at(n), ring) {
            (Some((x, y)), Some(r)) => r.inside(x, y),
            (Some(_), None) => true,
            (None, _) => false,
        };
        match self.phase {
            Phase::Signup | Phase::Done => Vec::new(),
            Phase::Waiting(t) if now >= t => {
                let Some(i) = self.next_match() else {
                    let top = self.standings().first().map(|s| s.0.clone());
                    return self.finish(top, now);
                };
                self.current = Some(i);
                self.phase = Phase::Called(now);
                self.out = [None, None];
                self.warned = false;
                let m = &self.matches[i];
                let twist = TWISTS.iter().find(|t| t.0 == self.twist).map(|t| format!(" House rule: {}.", t.1)).unwrap_or_default();
                vec![
                    Ev::All(format!(
                        "Next: {} vs {} (best of {}). {}{twist}",
                        m.a,
                        m.b,
                        self.best_of,
                        if self.ring.is_some() { "Fighters, into the ring!" } else { "Fighters, face each other!" }
                    )),
                    Ev::Refresh,
                ]
            }
            Phase::Waiting(_) => Vec::new(),
            Phase::Called(since) => {
                let Some((a, b)) = self.fighters() else { return Vec::new() };
                let (ra, rb) = (in_ring(&a, self.ring), in_ring(&b, self.ring));
                if ra && rb {
                    self.phase = Phase::Countdown(now + COUNT_MS, 0);
                    return vec![Ev::All(format!("{a} and {b} are ready. Get set...")), Ev::Refresh];
                }
                if now.saturating_sub(since) >= NO_SHOW_MS {
                    let (w, l) = if ra || (!rb && at(&a).is_some()) { (a, b) } else { (b, a) };
                    let i = self.current.unwrap_or(0);
                    self.matches[i].winner = Some(w.clone());
                    self.matches[i].how = "no-show".into();
                    self.current = None;
                    let mut ev = vec![Ev::All(format!("{l} didn't come to the ring: {w} goes through."))];
                    ev.extend(self.after_match(now));
                    ev.push(Ev::Refresh);
                    return ev;
                }
                Vec::new()
            }
            Phase::Countdown(go, said) => {
                if now >= go {
                    self.phase = Phase::Fight(now);
                    self.out = [None, None];
                    return vec![Ev::All("Fight!".into()), Ev::Refresh];
                }
                let left = ((go - now + 999) / 1000) as u8;
                if left >= 1 && left <= 3 && (said == 0 || left < said) {
                    self.phase = Phase::Countdown(go, left);
                    return vec![Ev::All(format!("{left}..."))];
                }
                Vec::new()
            }
            Phase::Fight(since) => {
                let Some((a, b)) = self.fighters() else { return Vec::new() };
                let mut ev = Vec::new();
                // ring outs (or gone from the Den): out this long loses the round
                for (k, n) in [a.clone(), b.clone()].iter().enumerate() {
                    if in_ring(n, self.ring) {
                        self.out[k] = None;
                        continue;
                    }
                    let first = *self.out[k].get_or_insert(now);
                    let gone = at(n).is_none();
                    let wait = if gone { NO_SHOW_MS / 6 } else { RING_OUT_MS };
                    if now.saturating_sub(first) >= wait {
                        let how = if gone { "left the Den" } else { "ring out" };
                        return self.round_lost(n, how, now).unwrap_or_default();
                    }
                }
                // spectators in the ring: asked once a match
                if let Some(r) = self.ring {
                    if !self.warned && here.iter().any(|h| !h.0.eq_ignore_ascii_case(&a) && !h.0.eq_ignore_ascii_case(&b) && r.inside(h.1, h.2)) {
                        self.warned = true;
                        ev.push(Ev::All("Spectators, please step out of the ring.".into()));
                    }
                }
                if !self.long_said && now.saturating_sub(since) >= LONG_ROUND_MS {
                    self.long_said = true;
                    ev.push(Ev::All(format!("A long round! {} can call it: .den tourney win <name>.", self.host)));
                }
                ev
            }
        }
    }

    pub fn phase_word(&self) -> &'static str {
        match self.phase {
            Phase::Signup => "signup",
            Phase::Waiting(_) => "waiting",
            Phase::Called(_) => "called",
            Phase::Countdown(_, _) => "countdown",
            Phase::Fight(_) => "fight",
            Phase::Done => "done",
        }
    }

    /// The Tournament window's status line (`wuden tourney`), under 200 characters: the names fit (12 letters). `me`:
/// host, in (signed up) or out; `in`: signed up (the host may be too).
    pub fn panel_line(&self, me: &str) -> String {
        let (a, b, wa, wb) = match self.current.map(|i| &self.matches[i]) {
            Some(m) => (m.a.as_str(), m.b.as_str(), m.wa, m.wb),
            None => ("-", "-", 0, 0),
        };
        let signed = self.players.iter().any(|p| p.eq_ignore_ascii_case(me));
        let role = if me.eq_ignore_ascii_case(&self.host) {
            "host"
        } else if signed {
            "in"
        } else {
            "out"
        };
        format!(
            "fmt={};bo={};phase={};n={};a={a};b={b};wa={wa};wb={wb};champ={};ring={};me={role};in={};twist={};title={}",
            self.format.word(),
            self.best_of,
            self.phase_word(),
            self.players.len(),
            self.champion.as_deref().unwrap_or("-"),
            self.ring.is_some() as u8,
            signed as u8,
            if self.twist.is_empty() { "-" } else { self.twist.as_str() },
            self.title.chars().take(32).collect::<String>(),
        )
    }

    /// The bracket for the window (`wuden tbr`): `<part>/<parts>|round,a,b,wa,wb,winner;...`, three matches a
    /// message (each under 200 characters). Then the sign-ups (`wuden tsign`): names, comma separated, in parts.
    pub fn bracket_parts(&self) -> Vec<String> {
        let rows: Vec<String> = self
            .matches
            .iter()
            .map(|m| format!("{},{},{},{},{},{}", m.round, m.a, if m.b.is_empty() { "-" } else { m.b.as_str() }, m.wa, m.wb, m.winner.as_deref().unwrap_or("-")))
            .collect();
        let chunks: Vec<String> = rows.chunks(3).map(|c| c.join(";")).collect();
        let n = chunks.len().max(1);
        if chunks.is_empty() {
            return vec!["1/1|".to_string()];
        }
        chunks.into_iter().enumerate().map(|(i, c)| format!("{}/{n}|{c}", i + 1)).collect()
    }

    pub fn signup_parts(&self) -> Vec<String> {
        let chunks: Vec<String> = self.players.chunks(12).map(|c| c.join(",")).collect();
        let n = chunks.len().max(1);
        if chunks.is_empty() {
            return vec!["1/1|".into()];
        }
        chunks.into_iter().enumerate().map(|(i, c)| format!("{}/{n}|{c}", i + 1)).collect()
    }

    /// The chat's version of the bracket (`.den tourney bracket`).
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![format!(
            "{}: {}, best of {}, {} players ({}).",
            self.title,
            self.format.title(),
            self.best_of,
            self.players.len(),
            match self.phase {
                Phase::Signup => "sign-ups open: .den tourney join",
                Phase::Done => "over",
                _ => "under way",
            }
        )];
        if self.matches.is_empty() {
            if !self.players.is_empty() {
                out.push(format!("Signed up: {}.", self.players.join(", ")));
            }
            return out;
        }
        let mut round = 0;
        let mut line = String::new();
        for (i, m) in self.matches.iter().enumerate() {
            if m.round != round {
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                round = m.round;
                line = if self.format == Format::King { String::new() } else { format!("Round {round}: ") };
            } else {
                line.push_str(", ");
            }
            let now = if self.current == Some(i) { " (now)" } else { "" };
            line.push_str(&match (&m.winner, m.b.is_empty()) {
                (_, true) => format!("{} (a bye)", m.a),
                (Some(w), _) => format!("{} vs {}: {w} {}-{}", m.a, m.b, m.wa.max(m.wb), m.wa.min(m.wb)),
                (None, _) => format!("{} vs {}{now}", m.a, m.b),
            });
        }
        if !line.is_empty() {
            out.push(line);
        }
        if self.format == Format::Robin {
            let t: Vec<String> = self.standings().iter().map(|s| format!("{} {}-{}", s.0, s.1, s.2)).collect();
            out.push(format!("Table: {}.", t.join(", ")));
        }
        if let Some(c) = &self.champion {
            out.push(format!("Champion: {c}."));
        }
        out
    }

    /// What goes in the Den file (`tournaments`).
    pub fn result(&self, unix: u64) -> Value {
        json!({
            "title": self.title,
            "format": self.format.word(),
            "best_of": self.best_of,
            "twist": self.twist,
            "players": self.players,
            "champion": self.champion,
            "at": unix,
            "matches": self.matches.iter().map(|m| json!({"round": m.round, "a": m.a, "b": m.b, "wa": m.wa, "wb": m.wb, "winner": m.winner, "how": m.how})).collect::<Vec<_>>(),
            "table": self.standings().iter().map(|s| json!({"name": s.0, "won": s.1, "lost": s.2})).collect::<Vec<_>>(),
        })
    }
}

/// The bracket's places for `size` seeds (0 the best): 1v8, 4v5, 2v7, 3v6 for eight, so the top two seeds can only
/// meet in the final.
pub fn seed_order(size: usize) -> Vec<usize> {
    let mut o = vec![0usize];
    while o.len() < size {
        let n = o.len() * 2;
        o = o.iter().flat_map(|&s| [s, n - 1 - s]).collect();
    }
    o
}

/// One single elimination round. Round 1: the players in signup order are the seeds, placed as [`seed_order`] says;
/// the top seeds get the byes. Later rounds: the winners in bracket order, one against the next.
pub fn single_round(players: &[String], round: u32) -> Vec<Match> {
    let n = players.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    if round == 1 {
        let order = seed_order(n.next_power_of_two());
        for pair in order.chunks(2) {
            let a = players[pair[0]].clone();
            match pair.get(1).filter(|j| **j < n) {
                Some(j) => out.push(Match { round, a, b: players[*j].clone(), wa: 0, wb: 0, winner: None, how: String::new() }),
                None => out.push(Match { round, a: a.clone(), b: String::new(), wa: 0, wb: 0, winner: Some(a), how: "bye".into() }),
            }
        }
    } else {
        for pair in players.chunks(2) {
            if pair.len() == 2 {
                out.push(Match { round, a: pair[0].clone(), b: pair[1].clone(), wa: 0, wb: 0, winner: None, how: String::new() });
            } else {
                out.push(Match { round, a: pair[0].clone(), b: String::new(), wa: 0, wb: 0, winner: Some(pair[0].clone()), how: "bye".into() });
            }
        }
    }
    out
}

/// Everyone against everyone (the circle method): rounds where nobody fights twice.
pub fn robin(players: &[String]) -> Vec<Match> {
    let mut p: Vec<String> = players.to_vec();
    if p.len() % 2 == 1 {
        p.push(String::new());
    }
    let n = p.len();
    let mut out = Vec::new();
    for r in 0..n.saturating_sub(1) {
        for i in 0..n / 2 {
            let (a, b) = (&p[i], &p[n - 1 - i]);
            if !a.is_empty() && !b.is_empty() {
                out.push(Match { round: r as u32 + 1, a: a.clone(), b: b.clone(), wa: 0, wb: 0, winner: None, how: String::new() });
            }
        }
        // keep the first, turn the rest one place
        let last = p.pop().unwrap_or_default();
        p.insert(1, last);
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// the glue: the Den's one tournament, its commands and its ticker (hosting.rs runs it)
// ------------------------------------------------------------------------------------------------------------------

/// The Den's tournament (one at a time; Den Lite runs one Den). Kept until the host makes a new one or clears it.
static T: Mutex<Option<Tourney>> = Mutex::new(None);

fn unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Every player in the Den now: (name, x, y), their writer and guid.
fn room(shared: &crate::Shared) -> Vec<crate::world::Peer> {
    shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn send(shared: &crate::Shared, t: &Tourney, ev: Vec<Ev>) {
    let peers = room(shared);
    for e in ev {
        match e {
            Ev::All(line) => {
                for p in &peers {
                    let _ = crate::world::say_to(&p.w, &line);
                }
            }
            Ev::Refresh => {
                for p in &peers {
                    refresh_one(&p.w, p.me.guid, &p.me.name, t);
                }
            }
            Ev::Finished => save(shared, t),
        }
    }
}

fn refresh_one(w: &crate::world::Writer, guid: u64, name: &str, t: &Tourney) {
    let _ = crate::world::addon_to(w, guid, "wuden", "tourney", &t.panel_line(name));
    for part in t.signup_parts() {
        let _ = crate::world::addon_to(w, guid, "wuden", "tsign", &part);
    }
    for part in t.bracket_parts() {
        let _ = crate::world::addon_to(w, guid, "wuden", "tbr", &part);
    }
}

/// The results in the Den file (`tournaments`, newest last, the last [`KEEP_RESULTS`]), written by the autosave.
fn save(shared: &crate::Shared, t: &Tourney) {
    {
        let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let mut list: Vec<Value> = d.extra.get("tournaments").and_then(Value::as_array).cloned().unwrap_or_default();
        list.push(t.result(unix()));
        let n = list.len();
        list.drain(..n.saturating_sub(KEEP_RESULTS));
        d.extra.insert("tournaments".into(), Value::Array(list));
    }
    shared.autosave();
    shared.event("tourney_done", t.champion.as_deref().unwrap_or("-"), &format!("{} {} players", t.format.word(), t.players.len()));
}

/// The ticker's part (hosting.rs, every quarter second).
pub fn tick(shared: &crate::Shared, ms: u64) {
    let mut g = T.lock().unwrap_or_else(|e| e.into_inner());
    let Some(t) = g.as_mut() else { return };
    if matches!(t.phase, Phase::Signup | Phase::Done) {
        return;
    }
    let here: Vec<(String, f32, f32)> = room(shared).iter().map(|p| (p.me.name.clone(), p.pos.x, p.pos.y)).collect();
    let ev = t.tick(ms, &here);
    if !ev.is_empty() {
        let snap = t.clone();
        drop(g);
        send(shared, &snap, ev);
    }
}

/// A player left for good (kicked): out of the tournament.
pub fn left(shared: &crate::Shared, name: &str) {
    let mut g = T.lock().unwrap_or_else(|e| e.into_inner());
    let Some(t) = g.as_mut() else { return };
    let ev = t.leave(name, now_ms());
    let snap = t.clone();
    drop(g);
    send(shared, &snap, ev);
}

/// For `<dens>/den-lite.online` (the launcher's Friends list shows a tournament under way).
pub fn summary_json() -> Value {
    let g = T.lock().unwrap_or_else(|e| e.into_inner());
    match g.as_ref() {
        None => Value::Null,
        Some(t) => json!({ "title": t.title, "format": t.format.word(), "phase": t.phase_word(), "players": t.players.len(), "champion": t.champion }),
    }
}

/// `.den tourney ...` (and the Tournament window's buttons). `me`: who typed it; `pos`: where they stand. The lines
/// for them alone come back; what everyone hears is sent here.
pub fn cmd(shared: &crate::Shared, me: &crate::world::Me, x: f32, y: f32, words: &[String]) -> Vec<String> {
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    let host = me.host;
    let ms = now_ms();
    let mut g = T.lock().unwrap_or_else(|e| e.into_inner());
    let host_only = || vec!["Only the Den's host runs the tournament (you can join, leave, yield and watch).".to_string()];
    let mut ev: Vec<Ev> = Vec::new();
    if matches!(w(0), "" | "status" | "bracket" | "show") {
        let snap = g.clone();
        drop(g);
        return match snap {
            Some(t) => {
                refresh_one_for(shared, me, &t);
                t.lines()
            }
            None if host => vec!["No tournament yet. .den tourney new single|robin|king [bo1|bo3|bo5] [title], or the Tournament button on the Den bar.".into()],
            None => vec!["No tournament yet: your host makes one.".into()],
        };
    }
    let mine: Vec<String> = match w(0) {
        "new" | "make" => {
            if !host {
                return host_only();
            }
            if g.as_ref().is_some_and(|t| !matches!(t.phase, Phase::Signup | Phase::Done)) {
                return vec!["A tournament is under way: .den tourney cancel first.".into()];
            }
            let format = Format::from_word(w(1)).unwrap_or(Format::Single);
            let mut at = if Format::from_word(w(1)).is_some() { 2 } else { 1 };
            let bo = w(at).strip_prefix("bo").and_then(|n| n.parse::<u8>().ok());
            if bo.is_some() {
                at += 1;
            }
            let title = words.iter().skip(at).cloned().collect::<Vec<_>>().join(" ");
            let mut t = Tourney::new(&title, format, bo.unwrap_or(1), &me.name);
            // the ring stays where it was marked
            t.ring = g.as_ref().and_then(|o| o.ring);
            let line = format!("{} is open: {}, best of {}. Sign up with .den tourney join (or the Tournament window).", t.title, format.title(), t.best_of);
            ev.push(Ev::All(line));
            ev.push(Ev::Refresh);
            *g = Some(t);
            Vec::new()
        }
        "join" | "signup" | "in" => match g.as_mut() {
            None => vec!["No tournament to join yet.".into()],
            Some(t) => match t.join(&me.name) {
                Ok(line) => {
                    ev.push(Ev::All(line));
                    ev.push(Ev::Refresh);
                    Vec::new()
                }
                Err(e) => vec![e],
            },
        },
        "add" if host => match (g.as_mut(), clean_name(w(1))) {
            (Some(t), Some(n)) => match t.join(&n) {
                Ok(line) => {
                    ev.push(Ev::All(line));
                    ev.push(Ev::Refresh);
                    Vec::new()
                }
                Err(e) => vec![e],
            },
            (None, _) => vec!["Make one first: .den tourney new.".into()],
            (_, None) => vec!["Add who? .den tourney add <name>".into()],
        },
        "leave" | "out" => match g.as_mut() {
            Some(t) => {
                ev = t.leave(&me.name, ms);
                if ev.is_empty() { vec!["You aren't signed up.".into()] } else { Vec::new() }
            }
            None => vec!["No tournament now.".into()],
        },
        "remove" | "kick" if host => match g.as_mut() {
            Some(t) => {
                ev = t.leave(w(1), ms);
                if ev.is_empty() { vec![format!("{} isn't signed up.", w(1))] } else { Vec::new() }
            }
            None => vec!["No tournament now.".into()],
        },
        "ring" | "arena" if host => {
            let r = w(1).parse::<f32>().ok().filter(|r| r.is_finite()).map(|d| (d / 2.0).clamp(3.0, 15.0)).unwrap_or(8.0);
            let ring = if w(1) == "off" { None } else { Some(Ring { x, y, r }) };
            if g.is_none() {
                *g = Some(Tourney::new("", Format::Single, 1, &me.name));
            }
            if let Some(t) = g.as_mut() {
                t.ring = ring;
                ev.push(Ev::Refresh);
            }
            match ring {
                Some(r) => vec![format!("The ring is here: {:.0} yards across, around where you stand. Step out during a fight and you lose the round. .den dojo ring builds a fenced one to see it by.", r.r * 2.0)],
                None => vec!["No ring: fights end by a yield or your call.".into()],
            }
        }
        "twist" | "rule" if host => match g.as_mut() {
            Some(t) => {
                let want = w(1);
                if want == "off" || want == "none" {
                    t.twist.clear();
                    ev.push(Ev::Refresh);
                    vec!["No house rule.".into()]
                } else if let Some(tw) = TWISTS.iter().find(|tw| tw.0 == want) {
                    t.twist = tw.0.to_string();
                    ev.push(Ev::All(format!("House rule for {}: {}.", t.title, tw.1)));
                    ev.push(Ev::Refresh);
                    Vec::new()
                } else {
                    vec![format!("House rules: {} (or off).", TWISTS.iter().map(|t| t.0).collect::<Vec<_>>().join(", "))]
                }
            }
            None => vec!["Make one first: .den tourney new.".into()],
        },
        "start" | "go" if host => match g.as_mut() {
            Some(t) => match t.start(ms) {
                Ok(e) => {
                    ev = e;
                    Vec::new()
                }
                Err(e) => vec![e],
            },
            None => vec!["Make one first: .den tourney new.".into()],
        },
        "win" | "call" if host => match g.as_mut() {
            Some(t) => match clean_name(w(1)).ok_or_else(|| "Who won? .den tourney win <name>".to_string()).and_then(|n| t.round_to(&n, "the host's call", ms)) {
                Ok(e) => {
                    ev = e;
                    Vec::new()
                }
                Err(e) => vec![e],
            },
            None => vec!["No tournament now.".into()],
        },
        "yield" | "concede" | "lose" => match g.as_mut() {
            Some(t) => match t.round_lost(&me.name, &format!("{} yielded", me.name), ms) {
                Ok(e) => {
                    ev = e;
                    Vec::new()
                }
                Err(e) => vec![e],
            },
            None => vec!["No tournament now.".into()],
        },
        "cancel" | "clear" | "end" if host => match g.take() {
            Some(t) => {
                let line = format!("{} is called off.", t.title);
                let peers = room(shared);
                drop(g);
                for p in &peers {
                    let _ = crate::world::say_to(&p.w, &line);
                    let _ = crate::world::addon_to(&p.w, p.me.guid, "wuden", "tourney", "phase=none");
                }
                return Vec::new();
            }
            None => vec!["No tournament now.".into()],
        },
        "help" => vec![
            "Tournaments: .den tourney (the bracket), join, leave, yield.".into(),
            "The host: new single|robin|king [bo1|bo3|bo5|bo7] [title], ring [yards|off], twist <rule>, add <name>, remove <name>, start, win <name>, cancel.".into(),
        ],
        _ if !host => host_only(),
        _ => vec!["Tournaments: .den tourney help.".into()],
    };
    let snap = g.clone();
    drop(g);
    if let Some(t) = snap {
        send(shared, &t, ev);
    }
    mine
}

fn refresh_one_for(shared: &crate::Shared, me: &crate::world::Me, t: &Tourney) {
    for p in room(shared) {
        if p.me.guid == me.guid {
            refresh_one(&p.w, p.me.guid, &p.me.name, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize) -> Vec<String> {
        ["Rowan", "Briar", "Ash", "Fen", "Moss", "Reed", "Wren", "Sorrel", "Hazel", "Linden", "Alder"].iter().take(n).map(|s| s.to_string()).collect()
    }

    fn signed(format: Format, bo: u8, n: usize) -> Tourney {
        let mut t = Tourney::new("Spring Cup", format, bo, "Wren");
        for p in names(n) {
            t.join(&p).unwrap();
        }
        t
    }

    /// Runs the ticker until the next fight starts (everyone standing in the ring at 0,0).
    fn to_fight(t: &mut Tourney, now: &mut u64) {
        let here: Vec<(String, f32, f32)> = t.players.iter().map(|p| (p.clone(), 0.0, 0.0)).collect();
        for _ in 0..200 {
            *now += 250;
            t.tick(*now, &here);
            if matches!(t.phase, Phase::Fight(_) | Phase::Done) {
                return;
            }
        }
        panic!("no fight: {:?}", t.phase);
    }

    #[test]
    fn sign_ups() {
        let mut t = Tourney::new("", Format::Single, 4, "Wren");
        assert_eq!((t.title.as_str(), t.best_of), ("Wren's Single elimination", 1), "best of an even number: one");
        assert!(t.join("rowan").unwrap().contains("Rowan signed up"));
        assert!(t.join("ROWAN").is_err() && t.join("x").is_err() && t.join("Ro wan").is_err());
        assert!(t.start(0).is_err(), "two at least");
        t.join("Briar").unwrap();
        assert_eq!(t.leave("rowan", 0).len(), 2);
        assert_eq!(t.players, vec!["Briar".to_string()]);
        let mut r = Tourney::new("", Format::Robin, 1, "Wren");
        for p in names(10) {
            r.join(&p).unwrap();
        }
        assert!(r.join("Extra").is_err(), "round robin stops at ten");
    }

    #[test]
    fn single_elimination_with_byes_to_a_champion() {
        let mut t = signed(Format::Single, 3, 5);
        let ev = t.start(0).unwrap();
        // 5 players in an 8 bracket: seeds 1-3 get byes, 4 meets 5 (in seed 1's half)
        assert_eq!(seed_order(8), vec![0, 7, 3, 4, 1, 6, 2, 5]);
        assert_eq!(t.matches.iter().filter(|m| m.how == "bye").count(), 3);
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l.contains("Rowan goes through"))));
        let mut now = 0;
        to_fight(&mut t, &mut now);
        let (a, b) = t.fighters().unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("Fen", "Moss"));
        // best of 3: two rounds win it
        t.round_to("Moss", "yielded", now).unwrap();
        assert!(matches!(t.phase, Phase::Countdown(_, _)), "the next round counts down");
        let ev = t.round_lost("Fen", "Fen yielded", now).unwrap();
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l.starts_with("Moss wins the match against Fen, 2-0"))));
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l.starts_with("Round 2 is drawn: Rowan vs Moss, Briar vs Ash"))), "{ev:?}");
        // play it out: the first fighter always wins
        let mut guard = 0;
        while t.phase != Phase::Done && guard < 20 {
            guard += 1;
            to_fight(&mut t, &mut now);
            if t.phase == Phase::Done {
                break;
            }
            let (a, _) = t.fighters().unwrap();
            t.round_to(&a, "the host's call", now).unwrap();
            t.round_to(&a, "the host's call", now).unwrap();
        }
        assert_eq!(t.champion.as_deref(), Some("Rowan"));
        assert_eq!(t.matches.iter().map(|m| m.round).max(), Some(3), "8 -> 4 -> 2 -> 1");
        assert!(t.lines().last().unwrap().contains("Champion: Rowan"));
        let r = t.result(1234);
        assert_eq!((r["champion"].as_str(), r["format"].as_str(), r["best_of"].as_u64()), (Some("Rowan"), Some("single"), Some(3)));
    }

    #[test]
    fn round_robin_everyone_plays_everyone() {
        let m = robin(&names(5));
        assert_eq!(m.len(), 10, "5 choose 2");
        let mut pairs: Vec<(String, String)> = m.iter().map(|x| if x.a < x.b { (x.a.clone(), x.b.clone()) } else { (x.b.clone(), x.a.clone()) }).collect();
        pairs.sort();
        pairs.dedup();
        assert_eq!(pairs.len(), 10, "no pair twice");
        for r in 1..=5 {
            let mut seen: Vec<&String> = m.iter().filter(|x| x.round == r).flat_map(|x| [&x.a, &x.b]).collect();
            let n = seen.len();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), n, "nobody twice in round {r}");
        }
        let mut t = signed(Format::Robin, 1, 3);
        t.start(0).unwrap();
        let mut now = 0;
        // Briar wins every match she plays; Rowan beats Ash
        while t.phase != Phase::Done {
            to_fight(&mut t, &mut now);
            if t.phase == Phase::Done {
                break;
            }
            let (a, b) = t.fighters().unwrap();
            let w = if a == "Briar" || b == "Briar" { "Briar".to_string() } else if a == "Rowan" || b == "Rowan" { "Rowan".to_string() } else { a };
            t.round_to(&w, "call", now).unwrap();
        }
        assert_eq!(t.champion.as_deref(), Some("Briar"));
        let s = t.standings();
        assert_eq!((s[0].0.as_str(), s[0].1, s[0].2), ("Briar", 2, 0));
        assert_eq!((s[1].0.as_str(), s[1].1), ("Rowan", 1));
        assert!(t.lines().iter().any(|l| l.starts_with("Table: Briar 2-0")));
    }

    #[test]
    fn king_of_the_hill_three_in_a_row() {
        let mut t = signed(Format::King, 1, 4);
        t.start(0).unwrap();
        let mut now = 0;
        to_fight(&mut t, &mut now);
        assert_eq!(t.fighters().unwrap(), ("Rowan".to_string(), "Briar".to_string()));
        let ev = t.round_to("Briar", "call", now).unwrap();
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l == "Briar takes the hill!")));
        assert_eq!(t.queue, vec!["Fen".to_string(), "Rowan".to_string()], "the loser to the back; Ash is next");
        for want in ["Ash", "Fen"] {
            to_fight(&mut t, &mut now);
            assert_eq!(t.fighters().unwrap().1, want);
            t.round_to("Briar", "call", now).unwrap();
        }
        assert_eq!((t.phase, t.champion.as_deref()), (Phase::Done, Some("Briar")), "three in a row: crowned");
    }

    #[test]
    fn the_ring_the_countdown_and_no_shows() {
        let mut t = signed(Format::Single, 1, 2);
        t.ring = Some(Ring { x: 0.0, y: 0.0, r: 8.0 });
        t.start(0).unwrap();
        // called: Briar is outside the ring, so no countdown yet
        let here = vec![("Rowan".to_string(), 1.0, 1.0), ("Briar".to_string(), 30.0, 0.0), ("Wren".to_string(), 40.0, 0.0)];
        let ev = t.tick(PAUSE_MS, &here);
        assert!(matches!(&ev[0], Ev::All(l) if l.starts_with("Next: Rowan vs Briar (best of 1). Fighters, into the ring!")));
        t.tick(PAUSE_MS + 500, &here);
        assert!(matches!(t.phase, Phase::Called(_)));
        let here = vec![("Rowan".to_string(), 1.0, 1.0), ("Briar".to_string(), -2.0, 3.0), ("Wren".to_string(), 2.0, 0.0)];
        let ev = t.tick(PAUSE_MS + 1000, &here);
        assert!(matches!(&ev[0], Ev::All(l) if l.contains("are ready")));
        let said: Vec<String> = (1..=14).flat_map(|k| t.tick(PAUSE_MS + 1000 + k * 250, &here)).filter_map(|e| if let Ev::All(l) = e { Some(l) } else { None }).collect();
        assert_eq!(said[..3], ["3...".to_string(), "2...".to_string(), "1...".to_string()]);
        assert_eq!(said[3], "Fight!");
        assert!(said.iter().any(|l| l.starts_with("Spectators")), "Wren is in the ring: asked out once");
        // a step over the line and back isn't a ring out; staying out is
        let start = PAUSE_MS + 5000;
        let out = vec![("Rowan".to_string(), 1.0, 1.0), ("Briar".to_string(), 12.0, 0.0)];
        assert!(t.tick(start, &out).is_empty());
        assert!(t.tick(start + 500, &here).is_empty());
        t.tick(start + 600, &out);
        let ev = t.tick(start + 600 + RING_OUT_MS, &out);
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l.starts_with("Rowan wins the match against Briar, 1-0 (ring out)"))), "{ev:?}");
        assert_eq!(t.champion.as_deref(), Some("Rowan"));
        // a no-show
        let mut n = signed(Format::Single, 1, 2);
        n.start(0).unwrap();
        let here = vec![("Rowan".to_string(), 0.0, 0.0)];
        n.tick(PAUSE_MS, &here);
        let ev = n.tick(PAUSE_MS + NO_SHOW_MS, &here);
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l.starts_with("Briar didn't come to the ring: Rowan goes through"))));
        assert_eq!(n.champion.as_deref(), Some("Rowan"));
    }

    #[test]
    fn leaving_mid_way_and_the_windows_lines() {
        let mut t = signed(Format::Single, 3, 4);
        t.start(0).unwrap();
        let mut now = 0;
        to_fight(&mut t, &mut now);
        assert_eq!(t.fighters().unwrap(), ("Rowan".to_string(), "Fen".to_string()));
        let ev = t.leave("Fen", now);
        assert!(ev.iter().any(|e| matches!(e, Ev::All(l) if l == "Fen left the tournament.")));
        assert_eq!(t.matches[0].winner.as_deref(), Some("Rowan"));
        assert!(matches!(t.phase, Phase::Waiting(_)), "the next match is called");
        let line = t.panel_line("Rowan");
        assert!(line.len() < 200 && line.contains("fmt=single;bo=3;phase=waiting;n=4") && line.contains("me=in"), "{line}");
        assert!(t.panel_line("Wren").contains("me=host;in=0") && t.panel_line("Moss").contains("me=out") && line.contains("me=in;in=1"));
        let mut big = signed(Format::Single, 1, 11);
        for p in ["Abcdefghijkl", "Bcdefghijklm", "Cdefghijklmn", "Defghijklmno", "Efghijklmnop"] {
            big.join(p).unwrap();
        }
        big.start(0).unwrap();
        let parts = big.bracket_parts();
        assert!(parts.iter().all(|p| p.len() < 200), "every message fits");
        assert!(parts[0].starts_with("1/") && parts.len() == 3, "{parts:?}");
        assert!(big.signup_parts().iter().all(|p| p.len() < 200));
        assert_eq!(Tourney::new("", Format::Robin, 1, "Wren").bracket_parts(), vec!["1/1|".to_string()]);
    }

    #[test]
    fn sudden_death_final() {
        let mut t = signed(Format::Single, 3, 2);
        t.twist = "sudden".into();
        t.start(0).unwrap();
        let mut now = 0;
        to_fight(&mut t, &mut now);
        t.round_to("Briar", "call", now).unwrap();
        assert_eq!(t.champion.as_deref(), Some("Briar"), "the final is one round");
    }
}
