//! The Dojo: the Den's training grounds, the part Den Lite can do without a combat engine (owner 3 Nov: "the Dojo in
//! the Den ... initial training dummy and npc area ... fly around ... speak to Jaina ... she walks over and casts
//! spells while things load into existence").
//!
//! - `.den dojo` (or `.dojo`): a ring of fences, torches, banners, racks and benches beside you (the stamp
//!   [`DOJO_STAMP`], a normal stamp: saved with the Den, one `.build undo` takes it back), built by your companion
//!   piece by piece when she is out, with training dummies standing in it (display-only creatures: Den Lite has no
//!   combat, so they never fight or count damage; the full Den's `ov_dojo.lua` does that).
//! - `.den fly on|off`: the game lets the character fly (SMSG_MOVE_SET_CAN_FLY / UNSET, as AzerothCore's
//!   `Unit::SetCanFly` says it to a player: packed guid, a counter). Jump, then Space to rise and X to sink.
//! - `.den speed 1-5`: run and flight speed times 1 to 5 (SMSG_FORCE_RUN_SPEED_CHANGE / FORCE_FLIGHT_SPEED_CHANGE,
//!   as `Unit::SetSpeed`: packed guid, the counter the game echoes in its ack, for run a byte 0, the speed).
//! - `.den jaina build <stamp>` or "Jaina, build a garden" (or a tavern, a forge, a reading nook: stamps.rs NAMES):
//!   she walks into the stamp's middle (the free spot nearest it: never inside a piece), starts a cast, then each
//!   piece appears the way a person builds a place (stamps.rs `order`: the building, rugs and boards, the big pieces,
//!   furniture, small things, last what stands on a table), a big one taking longer than a small one
//!   (stamps.rs `pace_ms`, about [`PIECE_MS`]), while she turns to it and casts at it (SMSG_SPELL_START, then one
//!   SMSG_SPELL_GO per piece with the piece's spot as the target; the game draws the spell's visuals from its own
//!   Spell.dbc: no aura, no spell system here). Then she steps back on your side, turns to look at it
//!   ([`Step::Admire`]) and says so; [`ADMIRE_MS`] later she follows you again.
//!
//! The game's answers to the fly and speed changes (CMSG_MOVE_SET_CAN_FLY_ACK, CMSG_FORCE_*_SPEED_CHANGE_ACK: packed
//! guid, the counter, the movement info without its guid, then a float) are read for the events log and the
//! player's position; nothing waits on them.
//!
//! Everything here is plain logic and message bodies, tested below; world.rs sends them.

use crate::combat::Kind;
use crate::companion::At;
use crate::den::WorldObject;
use crate::raw::packed_guid;
use std::f32::consts::PI;

pub const SMSG_FORCE_RUN_SPEED_CHANGE: u16 = 0x0E2;
pub const CMSG_FORCE_RUN_SPEED_CHANGE_ACK: u32 = 0x0E3;
pub const SMSG_MOVE_SET_CAN_FLY: u16 = 0x343;
pub const SMSG_MOVE_UNSET_CAN_FLY: u16 = 0x344;
pub const CMSG_MOVE_SET_CAN_FLY_ACK: u32 = 0x345;
/// The game says it starts or stops flying (its movement info, like a move).
pub const CMSG_MOVE_SET_FLY: u32 = 0x346;
pub const SMSG_FORCE_FLIGHT_SPEED_CHANGE: u16 = 0x381;
pub const CMSG_FORCE_FLIGHT_SPEED_CHANGE_ACK: u32 = 0x382;
pub const SMSG_SPELL_START: u16 = 0x131;
pub const SMSG_SPELL_GO: u16 = 0x132;
pub const SMSG_MONSTER_MOVE: u16 = 0x0DD;
/// The acks read here (world.rs reads them before the message crate sees them).
pub const ACKS: [u32; 3] = [CMSG_MOVE_SET_CAN_FLY_ACK, CMSG_FORCE_RUN_SPEED_CHANGE_ACK, CMSG_FORCE_FLIGHT_SPEED_CHANGE_ACK];

/// The character's normal run and flight speeds (yards a second; the player's create says the same).
pub const BASE_RUN: f32 = 7.0;
pub const BASE_FLIGHT: f32 = 7.0;
pub const MAX_SPEED: u8 = 5;

/// The spell whose visuals she casts while building: a conjuring cast (any spell the game's own Spell.dbc has would
/// do; only its look is used, nothing of it happens). To see on the real game which looks best.
pub const BUILD_SPELL: u32 = 42955;
/// Between two pieces appearing.
pub const PIECE_MS: u64 = 250;
/// At most this long for her walk to the spot before she starts (a long walk is cut short: she starts on the way).
pub const WALK_MAX_MS: u64 = 4000;
/// How long she stands back to look at what she built before she follows you again.
pub const ADMIRE_MS: u64 = 2500;
/// TARGET_FLAG_DEST_LOCATION (SpellCastTargets).
const TARGET_DEST: u32 = 0x40;
/// CAST_FLAG_UNKNOWN_2 for a start, CAST_FLAG_UNKNOWN_9 for a go (as AzerothCore's Spell::SendSpellStart / Go).
const CAST_START: u32 = 0x2;
const CAST_GO: u32 = 0x100;

/// The training dummies Den Lite shows: the game's own training dummy creatures (AzerothCore `creature_template`
/// entries), display only: (entry, display id, name, level). The display id is to check on the real game (the
/// dummies of the capital cities); a wrong one shows another model, nothing worse.
pub const DUMMIES: &[(u32, u32, &str, u8)] = &[
    (31144, 27510, "Grandmaster's Training Dummy", 80),
    (32667, 27510, "Master's Training Dummy", 70),
    (32666, 27510, "Expert's Training Dummy", 60),
    (31146, 27510, "Raider's Training Dummy", 83),
];
/// How many dummies `.den dojo dummies` puts up at most.
pub const MAX_DUMMIES: usize = 5;

/// The sandbox combat's other dummies (combat.rs Kind): Den Lite's own entries (their names come from this server's
/// creature query answer), the training dummy's model; the boss twice its size.
pub const KIND_DUMMIES: &[(u32, u32, &str, u8, Kind)] = &[
    (900_001, 27510, "Pack Dummy", 80, Kind::Pack),
    (900_002, 27510, "Moving Dummy", 80, Kind::Mover),
    (900_003, 27510, "Wounded Dummy", 80, Kind::Healer),
    (900_004, 27510, "Sparring Dummy", 80, Kind::Tank),
    (900_005, 27510, "Practice Boss", 83, Kind::Boss),
];

pub fn dummy(entry: u32) -> Option<(u32, u32, &'static str, u8)> {
    DUMMIES.iter().copied().find(|d| d.0 == entry).or_else(|| KIND_DUMMIES.iter().find(|d| d.0 == entry).map(|d| (d.0, d.1, d.2, d.3)))
}

/// A dummy's kind by its entry (the training dummies: Target).
pub fn kind_of(entry: u32) -> Kind {
    KIND_DUMMIES.iter().find(|d| d.0 == entry).map(|d| d.4).unwrap_or(Kind::Target)
}

/// The entry a kind goes up with.
pub fn entry_of(kind: Kind) -> u32 {
    KIND_DUMMIES.iter().find(|d| d.4 == kind).map(|d| d.0).unwrap_or(DUMMIES[0].0)
}

/// How many of a kind go up, and where (x ahead, y left of you or the ring's middle): five in a tight pack, one
/// walker, the wounded one to your left, the sparring one close, the boss further out.
pub fn kind_spots(kind: Kind) -> Vec<(f32, f32)> {
    match kind {
        Kind::Pack => vec![(6.0, 0.0), (6.8, 1.2), (6.8, -1.2), (7.6, 0.6), (7.6, -0.6)],
        Kind::Mover => vec![(MOVER_RADIUS + 3.0, 0.0)],
        Kind::Healer => vec![(2.0, 3.0)],
        Kind::Tank => vec![(3.0, 0.0)],
        Kind::Boss => vec![(12.0, 0.0)],
        _ => vec![(5.0, 0.0)],
    }
}

/// The moving dummy walks a circle this wide around where it went up, a step every MOVER_STEP_MS.
pub const MOVER_RADIUS: f32 = 4.0;
pub const MOVER_STEP_MS: u64 = 2000;
pub const MOVER_POINTS: usize = 8;

/// The moving dummy's next point on its circle around `home` (point `n`).
pub fn mover_point(home: At, n: usize) -> At {
    let a = n as f32 * 2.0 * PI / MOVER_POINTS as f32;
    At { x: home.x + a.cos() * MOVER_RADIUS, y: home.y + a.sin() * MOVER_RADIUS, z: home.z, o: (a + PI / 2.0).rem_euclid(2.0 * PI) }
}

/// A creature guid (HighGuid::Unit 0xF130, the entry, a counter), as the companion's.
pub fn creature_guid(entry: u32, n: u32) -> u64 {
    (0xF130u64 << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (n as u64 & 0xFF_FFFF)
}

/// The training ring (a stamp: x ahead of you, y to your left; entries from objects.rs). The opening faces you.
/// TODO(owner): the ring's look (which fences, banners, how big).
pub const DOJO_STAMP: &str = r#"{"format": "leyline-den", "version": 1, "kind": "stamp", "name": "dojo", "title": "The Dojo",
 "line": "A training ring: fences, torches, banners, weapon racks and benches.",
 "props": [
  {"entry": 19434, "x": 6.0, "y": 0.0, "z": 0, "yaw": 90},
  {"entry": 19434, "x": 5.196, "y": 3.0, "z": 0, "yaw": 120},
  {"entry": 19434, "x": 3.0, "y": 5.196, "z": 0, "yaw": 150},
  {"entry": 19434, "x": 0.0, "y": 6.0, "z": 0, "yaw": 0},
  {"entry": 19434, "x": -3.0, "y": 5.196, "z": 0, "yaw": 30},
  {"entry": 19434, "x": -5.196, "y": 3.0, "z": 0, "yaw": 60},
  {"entry": 19434, "x": -5.196, "y": -3.0, "z": 0, "yaw": 120},
  {"entry": 19434, "x": -3.0, "y": -5.196, "z": 0, "yaw": 150},
  {"entry": 19434, "x": 0.0, "y": -6.0, "z": 0, "yaw": 0},
  {"entry": 19434, "x": 3.0, "y": -5.196, "z": 0, "yaw": 30},
  {"entry": 19434, "x": 5.196, "y": -3.0, "z": 0, "yaw": 60},
  {"entry": 180043, "x": 4.95, "y": 4.95, "z": 0, "yaw": 0},
  {"entry": 180043, "x": -4.95, "y": 4.95, "z": 0, "yaw": 0},
  {"entry": 180043, "x": -4.95, "y": -4.95, "z": 0, "yaw": 0},
  {"entry": 180043, "x": 4.95, "y": -4.95, "z": 0, "yaw": 0},
  {"entry": 180773, "x": -6.9, "y": 1.6, "z": 0, "yaw": 180},
  {"entry": 180774, "x": -6.9, "y": -1.6, "z": 0, "yaw": 180},
  {"entry": 19635, "x": 2.5, "y": 3.6, "z": 0, "yaw": 270},
  {"entry": 19635, "x": 2.5, "y": -3.6, "z": 0, "yaw": 90},
  {"entry": 180042, "x": 7.6, "y": 0.0, "z": 0, "yaw": 180},
  {"entry": 24538, "x": -8.0, "y": 3.5, "z": 0, "yaw": 0},
  {"entry": 24538, "x": -8.0, "y": -3.5, "z": 0, "yaw": 0}
 ]}"#;

/// Where the dummies stand in the ring (x ahead, y left of its middle), facing its opening.
pub const DUMMY_SPOTS: &[(f32, f32)] = &[(3.0, 0.0), (3.0, 1.8), (3.0, -1.8), (1.2, 2.6), (1.2, -2.6)];

/// One dummy in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dummy {
    pub guid: u64,
    pub entry: u32,
    pub at: At,
    /// What it does in the sandbox combat (combat.rs).
    pub kind: Kind,
    /// Where it went up (the moving dummy's circle's middle) and its step.
    pub home: At,
    pub step: usize,
}

/// The Dojo's state (one: the host's).
#[derive(Debug, Default)]
pub struct Dojo {
    pub fly: bool,
    /// 1 to MAX_SPEED (0: never set, the same as 1).
    pub speed: u8,
    counter: u32,
    pub dummies: Vec<Dummy>,
    next_dummy: u32,
    /// The ring's middle and the way it faces, when one was laid out in this run.
    pub ring: Option<At>,
    /// A build in progress (Jaina's).
    pub job: Option<Job>,
    /// The game's acks seen: (fly, speed).
    pub acks: (u32, u32),
}

impl Dojo {
    /// The next movement counter (the game echoes it in its ack).
    pub fn counter(&mut self) -> u32 {
        self.counter = self.counter.wrapping_add(1);
        self.counter
    }

    /// A new time in the world: the game starts on foot at the normal speed; the dummies are sent again by world.rs.
    pub fn entered(&mut self) {
        self.fly = false;
        self.speed = 1;
        self.job = None;
    }

    pub fn speed_now(&self) -> u8 {
        self.speed.max(1)
    }

    /// `n` dummies (1 to MAX_DUMMIES) in the ring when there is one, else in a row 5 yards in front of `p`, facing
    /// back, standing at `z` (the ground's height; world.rs takes the old ones down first with `clear_dummies`).
    pub fn place_dummies(&mut self, p: At, n: usize, entry: u32, z: f32) -> Vec<Dummy> {
        let n = n.clamp(1, MAX_DUMMIES);
        let made: Vec<Dummy> = match self.ring {
            Some(r) => DUMMY_SPOTS[..n]
                .iter()
                .map(|&(x, y)| {
                    let w = local(r, x, y);
                    At { z, o: (r.o + PI).rem_euclid(2.0 * PI), ..w }
                })
                .collect::<Vec<_>>(),
            None => {
                let mid = ahead(p, 5.0);
                (0..n)
                    .map(|i| {
                        let side = (i as f32 - (n as f32 - 1.0) / 2.0) * 1.8;
                        At { z, o: (p.o + PI).rem_euclid(2.0 * PI), ..local(mid, 0.0, side) }
                    })
                    .collect()
            }
        }
        .into_iter()
        .map(|at| {
            self.next_dummy += 1;
            Dummy { guid: creature_guid(entry, 0x100 + self.next_dummy), entry, at, kind: kind_of(entry), home: at, step: 0 }
        })
        .collect();
        self.dummies.extend(made.iter().copied());
        made
    }

    /// The dummies of one kind (combat.rs Kind) in front of `p` (or in the ring), standing at `z`; the old ones of
    /// that kind are taken down first: their guids, and the new ones.
    pub fn place_kind(&mut self, p: At, kind: Kind, z: f32) -> (Vec<u64>, Vec<Dummy>) {
        let gone: Vec<u64> = self.dummies.iter().filter(|d| d.kind == kind).map(|d| d.guid).collect();
        self.dummies.retain(|d| d.kind != kind);
        let base = self.ring.map(|r| At { x: r.x - r.o.cos() * 4.0, y: r.y - r.o.sin() * 4.0, ..r }).unwrap_or(p);
        let entry = entry_of(kind);
        let made: Vec<Dummy> = kind_spots(kind)
            .into_iter()
            .map(|(x, y)| {
                let w = local(base, x, y);
                self.next_dummy += 1;
                let at = At { z, o: (base.o + PI).rem_euclid(2.0 * PI), ..w };
                Dummy { guid: creature_guid(entry, 0x100 + self.next_dummy), entry, at, kind, home: at, step: 0 }
            })
            .collect();
        self.dummies.extend(made.iter().copied());
        (gone, made)
    }

    /// The moving dummies' next steps due (every MOVER_STEP_MS): (guid, from, to). Their places are kept.
    pub fn mover_steps(&mut self) -> Vec<(u64, At, At)> {
        let mut out = Vec::new();
        for d in self.dummies.iter_mut().filter(|d| d.kind == Kind::Mover) {
            d.step = (d.step + 1) % MOVER_POINTS;
            let to = mover_point(At { x: d.home.x - MOVER_RADIUS, ..d.home }, d.step);
            out.push((d.guid, d.at, to));
            d.at = to;
        }
        out
    }

    /// Every dummy taken down: their guids.
    pub fn clear_dummies(&mut self) -> Vec<u64> {
        self.dummies.drain(..).map(|d| d.guid).collect()
    }

    /// The Den bar's Dojo line: `server=lite;fly=0|1;speed=<n>;dummies=<n>;ring=0|1;building=0|1`.
    pub fn line(&self) -> String {
        format!(
            "server=lite;fly={};speed={};dummies={};ring={};building={}",
            self.fly as u8,
            self.speed_now(),
            self.dummies.len(),
            self.ring.is_some() as u8,
            self.job.is_some() as u8
        )
    }
}

/// A spot `yards` ahead of `p` (same facing).
pub fn ahead(p: At, yards: f32) -> At {
    At { x: p.x + p.o.cos() * yards, y: p.y + p.o.sin() * yards, ..p }
}

/// A spot at (x ahead, y left) of `p`, as stamps count.
pub fn local(p: At, x: f32, y: f32) -> At {
    let (s, c) = p.o.sin_cos();
    At { x: p.x + x * c - y * s, y: p.y + x * s + y * c, ..p }
}

/// The facing from `a` towards (x, y).
pub fn facing(a: At, x: f32, y: f32) -> f32 {
    (y - a.y).atan2(x - a.x).rem_euclid(2.0 * PI)
}

/// `.den speed <n>`: 1 to 5.
pub fn speed_arg(s: &str) -> Result<u8, String> {
    match s.trim_end_matches('x').parse::<u8>() {
        Ok(n) if (1..=MAX_SPEED).contains(&n) => Ok(n),
        _ => Err(format!("Speeds are 1 to {MAX_SPEED}: .den speed 2 (times your normal run); .den speed 1 is normal.")),
    }
}

/// `on`/`off` (and a few friendlier words); None: neither.
pub fn on_off(s: &str) -> Option<bool> {
    match s {
        "on" | "yes" | "1" | "start" => Some(true),
        "off" | "no" | "0" | "stop" | "land" => Some(false),
        _ => None,
    }
}

/// "build a garden", "build me the inn", "make a campfire", "build the dojo", "build a tavern" (the inn: stamps.rs
/// NAMES): the stamp's name (an unknown word as it was, so the answer can say there is none); None: not a build.
pub fn build_words(words: &str) -> Option<String> {
    let low = words.to_lowercase();
    let mut it = low.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_').filter(|w| !w.is_empty());
    if !matches!(it.next()?, "build" | "make" | "conjure" | "raise") {
        return None;
    }
    it.find(|w| {
        !matches!(*w, "a" | "an" | "the" | "me" | "us" | "my" | "our" | "please" | "some" | "little" | "small" | "up" | "cozy" | "cosy"
            | "nice" | "new" | "quiet" | "old")
    })
    .map(|w| crate::stamps::canonical(w).map(str::to_string).unwrap_or_else(|| w.to_string()))
}

// ---------------------------------------------------------------- message bodies

/// SMSG_MOVE_SET_CAN_FLY / SMSG_MOVE_UNSET_CAN_FLY: packed guid, the counter.
pub fn can_fly_body(guid: u64, counter: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(13);
    packed_guid(&mut b, guid);
    b.extend_from_slice(&counter.to_le_bytes());
    b
}

/// SMSG_FORCE_RUN_SPEED_CHANGE: packed guid, the counter, a byte 0 (run only), the speed.
pub fn run_speed_body(guid: u64, counter: u32, speed: f32) -> Vec<u8> {
    let mut b = Vec::with_capacity(18);
    packed_guid(&mut b, guid);
    b.extend_from_slice(&counter.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&speed.to_le_bytes());
    b
}

/// SMSG_FORCE_FLIGHT_SPEED_CHANGE: packed guid, the counter, the speed.
pub fn flight_speed_body(guid: u64, counter: u32, speed: f32) -> Vec<u8> {
    let mut b = Vec::with_capacity(17);
    packed_guid(&mut b, guid);
    b.extend_from_slice(&counter.to_le_bytes());
    b.extend_from_slice(&speed.to_le_bytes());
    b
}

/// An ack's counter (packed guid, u32 counter, ...).
pub fn ack_counter(body: &[u8]) -> Option<u32> {
    let at = 1 + body.first()?.count_ones() as usize;
    body.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Where an ack says the character is: after the packed guid and the counter, the movement info without a guid
/// (u32 flags, u16 flags2, u32 time, then x, y, z, orientation).
pub fn ack_pos(body: &[u8]) -> Option<At> {
    let at = 1 + body.first()?.count_ones() as usize + 4 + 4 + 2 + 4;
    let f = |k: usize| body.get(at + 4 * k..at + 4 * k + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (x, y, z, o) = (f(0)?, f(1)?, f(2)?, f(3)?);
    [x, y, z, o].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some(At { x, y, z, o })
}

fn dest(out: &mut Vec<u8>, at: [f32; 3]) {
    out.extend_from_slice(&TARGET_DEST.to_le_bytes());
    out.push(0); // the transport's packed guid: none
    for v in at {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// SMSG_SPELL_START from a creature at a place (AzerothCore Spell::SendSpellStart): the caster twice (packed), the
/// cast count, the spell, the flags, the cast time, the targets.
pub fn spell_start_body(caster: u64, spell: u32, cast_ms: u32, at: [f32; 3]) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, caster);
    packed_guid(&mut b, caster);
    b.push(0);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&CAST_START.to_le_bytes());
    b.extend_from_slice(&cast_ms.to_le_bytes());
    dest(&mut b, at);
    b
}

/// SMSG_SPELL_GO (Spell::SendSpellGo): the caster twice, the cast count, the spell, the flags, the time, no units hit
/// and none missed, the targets, and the byte that follows a place target.
pub fn spell_go_body(caster: u64, spell: u32, time_ms: u32, at: [f32; 3]) -> Vec<u8> {
    let mut b = Vec::with_capacity(52);
    packed_guid(&mut b, caster);
    packed_guid(&mut b, caster);
    b.push(0);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&CAST_GO.to_le_bytes());
    b.extend_from_slice(&time_ms.to_le_bytes());
    b.push(0); // hit
    b.push(0); // missed
    dest(&mut b, at);
    b.push(0);
    b
}

/// SMSG_MONSTER_MOVE turning a creature where it stands (MonsterMoveType 4, facing angle): packed guid, a byte, the
/// start, the spline id, the type and the angle, no spline flags, a short duration, one point (where it stands).
pub fn facing_body(guid: u64, at: At, id: u32, angle: f32) -> Vec<u8> {
    walk_facing_body(guid, at, at, id, 100, angle)
}

/// SMSG_MONSTER_MOVE walking a creature from `from` to `to` in `ms` and turning it to `angle` when it gets there
/// (MonsterMoveType 4, as [`facing_body`]: she walks back to look at her build and ends facing it, not the way she
/// walked).
pub fn walk_facing_body(guid: u64, from: At, to: At, id: u32, ms: u32, angle: f32) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, guid);
    b.push(0);
    for v in [from.x, from.y, from.z] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&id.to_le_bytes());
    b.push(4);
    b.extend_from_slice(&angle.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&ms.max(1).to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    for v in [to.x, to.y, to.z] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

// ---------------------------------------------------------------- Jaina builds a set

/// Where she stands to build what is at `origin` (laid out in front of a player at `p`): between the two, a little
/// to the player's left, facing the middle.
pub fn stand_spot(p: At, origin: At) -> At {
    let mid = At { x: (p.x + origin.x) / 2.0, y: (p.y + origin.y) / 2.0, z: p.z, o: p.o };
    let s = local(mid, 0.0, 1.5);
    At { o: facing(s, origin.x, origin.y), ..s }
}

/// Where she builds from: the free spot nearest the stamp's middle (`origin`), among its pieces (stamps.rs).
pub fn build_stand(origin: At, pieces: &[WorldObject]) -> At {
    crate::stamps::clear_spot(origin, pieces)
}

/// Where she looks at it from when it is done: a step back on the player's side, free, facing it (stamps.rs).
pub fn admire_spot(origin: At, pieces: &[WorldObject], player: At) -> At {
    crate::stamps::admire_spot(origin, pieces, player)
}

/// How far ahead of the player a stamp's middle goes so the whole stamp is in front of them: its reach and 1.5.
pub fn reach(pieces: &[(f64, f64)]) -> f32 {
    pieces.iter().map(|(x, y)| (x * x + y * y).sqrt()).fold(0.0, f64::max) as f32 + 1.5
}

/// What a build does next.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// She starts casting (for `ms`), at the middle.
    Start { ms: u32, at: [f32; 3] },
    /// One piece appears; she turns to it (facing) and casts at it.
    Piece { obj: WorldObject, facing: f32 },
    /// The last piece is up: she walks back to `to` and ends facing the build, and says it is done.
    Admire { to: At },
    /// All of them are there (how many appeared; the rest were taken back while she built). `admired`: she looked at
    /// it already (and said so); else this is when she says it.
    Done { shown: usize, admired: bool },
}

/// A build in progress: the pieces are in the Den already (saved, one undo); the game is shown them one by one.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub stand: At,
    pub origin: At,
    /// In build order (stamps.rs `order`), each with the time it takes ([`Job::paces`]).
    pub pieces: Vec<WorldObject>,
    pub paces: Vec<u64>,
    pub next: usize,
    pub shown: usize,
    /// When she starts casting (after her walk), then when the next piece is due (milliseconds).
    pub start_at: u64,
    pub next_at: u64,
    pub started: bool,
    /// Dummies go up when it is done (`.den dojo`).
    pub then_dummies: bool,
    pub title: String,
    /// Where she steps back to look at it when it is done (None: she doesn't, Done comes with the last piece).
    pub admire: Option<At>,
    /// When the look is over (set when she steps back).
    pub admire_until: Option<u64>,
}

impl Job {
    /// A build whose pieces are `pieces` (in build order: big first, then nearest her), she at `stand` from `ms` +
    /// her walk; `origin.z` is the ground there (a piece above it stands on something).
    pub fn new(stand: At, origin: At, mut pieces: Vec<WorldObject>, ms: u64, walk_ms: u64, title: &str) -> Job {
        use crate::stamps::{order, pace_ms, size_of, tier};
        order(&mut pieces, stand, origin.z);
        let mut paces: Vec<u64> = pieces.iter().map(|o| pace_ms(tier(&size_of(o), o.z > origin.z + 0.2))).collect();
        // hundreds of pieces (a copied place): quicker each, the whole in BUILD_MAX_MS
        let sum: u64 = paces.iter().sum();
        if sum > crate::stamps::BUILD_MAX_MS {
            for p in &mut paces {
                *p = (*p * crate::stamps::BUILD_MAX_MS / sum).max(20);
            }
        }
        let start = ms + walk_ms.min(WALK_MAX_MS);
        Job {
            stand,
            origin,
            pieces,
            paces,
            next: 0,
            shown: 0,
            start_at: start,
            next_at: start + PIECE_MS,
            started: false,
            then_dummies: false,
            title: title.into(),
            admire: None,
            admire_until: None,
        }
    }

    /// How long the whole cast takes once she starts.
    pub fn cast_ms(&self) -> u32 {
        (self.paces.iter().sum::<u64>() + PIECE_MS) as u32
    }

    /// What is due at `ms`; `still(guid)`: the piece is still in the Den (an undo while she builds takes the rest
    /// back: those never appear). After `Done` the job is over.
    pub fn due(&mut self, ms: u64, still: impl Fn(u64) -> bool) -> Vec<Step> {
        let mut out = Vec::new();
        if ms < self.start_at {
            return out;
        }
        if !self.started {
            self.started = true;
            out.push(Step::Start { ms: self.cast_ms(), at: [self.origin.x, self.origin.y, self.origin.z] });
        }
        while self.next < self.pieces.len() && ms >= self.next_at {
            let obj = self.pieces[self.next].clone();
            self.next_at += self.paces.get(self.next).copied().unwrap_or(PIECE_MS);
            self.next += 1;
            if still(obj.guid) {
                self.shown += 1;
                out.push(Step::Piece { facing: facing(self.stand, obj.x, obj.y), obj });
            }
        }
        if self.next < self.pieces.len() {
            return out;
        }
        match (self.admire, self.admire_until) {
            // the last piece has had its time: she steps back to look at it
            (Some(to), None) if self.shown > 0 => {
                if ms >= self.next_at {
                    self.admire_until = Some(ms + ADMIRE_MS);
                    out.push(Step::Admire { to });
                }
            }
            (Some(_), Some(until)) => {
                if ms >= until {
                    out.push(Step::Done { shown: self.shown, admired: true });
                }
            }
            _ => out.push(Step::Done { shown: self.shown, admired: false }),
        }
        out
    }
}

/// Her lines while she builds. TODO(owner): a voice pass.
pub const ON_IT: &str = "Leave it to me. Stand back a little.";
pub const BUILT: &str = "There. What shall we try next?";
pub const BUILT_DOJO: &str = "Your Dojo is ready. The dummies won't hit back.";
pub const NOT_OUT: &str = "Call your companion first (.den companion): she builds it for you. Or .den stamp <name> now.";
pub const BUSY: &str = "One moment, I'm still building.";
/// What the full Den does that Den Lite can't (no combat engine).
pub const FULL_ONLY: &str = "needs the full Den (Leyline > Host > Your Den): Den Lite has no combat";

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(guid: u64, x: f32, y: f32) -> WorldObject {
        WorldObject { guid, entry: 19434, display: 1, kind: 5, scale: 1.0, x, y, z: 13.2, o: 0.0, pitch: 0.0, roll: 0.0 }
    }

    #[test]
    fn the_dojo_stamp_reads_as_a_stamp_of_known_pieces() {
        let doc = crate::den::parse(DOJO_STAMP).expect("the Dojo stamp parses");
        assert!(doc.props.len() >= 20 && doc.props.len() <= crate::den::STAMP_MAX);
        for p in &doc.props {
            assert!(crate::den::template(p.entry as u32).is_some(), "entry {} is one Den Lite knows", p.entry);
            assert!(p.x.abs() <= 10.0 && p.y.abs() <= 10.0);
        }
        // the opening faces you: no fence on the near side (x < -5, |y| < 2)
        assert!(!doc.props.iter().any(|p| p.entry == 19434 && p.x < -5.0 && p.y.abs() < 2.0));
        let r = reach(&doc.props.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>());
        assert!(r > 9.0 && r < 11.0, "{r}");
        assert!(!crate::companion::branded(DOJO_STAMP));
    }

    #[test]
    fn speeds_and_switches() {
        assert_eq!(speed_arg("3"), Ok(3));
        assert_eq!(speed_arg("2x"), Ok(2));
        assert!(speed_arg("0").is_err() && speed_arg("6").is_err() && speed_arg("fast").is_err());
        assert_eq!(on_off("on"), Some(true));
        assert_eq!(on_off("land"), Some(false));
        assert_eq!(on_off("maybe"), None);
    }

    #[test]
    fn build_words_find_the_stamp() {
        assert_eq!(build_words("build a garden").as_deref(), Some("garden"));
        assert_eq!(build_words("Build me the inn, please").as_deref(), Some("inn"));
        assert_eq!(build_words("make a little campfire").as_deref(), Some("campfire"));
        assert_eq!(build_words("conjure the dojo!").as_deref(), Some("dojo"));
        assert_eq!(build_words("how do I build a house"), None);
        assert_eq!(build_words("build"), None);
    }

    #[test]
    fn fly_and_speed_bodies() {
        // packed guid 1: mask 0x01, byte 0x01; then the counter
        assert_eq!(can_fly_body(1, 7), vec![0x01, 0x01, 7, 0, 0, 0]);
        let r = run_speed_body(1, 2, 14.0);
        assert_eq!(&r[..7], &[0x01, 0x01, 2, 0, 0, 0, 0]);
        assert_eq!(f32::from_le_bytes([r[7], r[8], r[9], r[10]]), 14.0);
        assert_eq!(r.len(), 11);
        let f = flight_speed_body(1, 3, 21.0);
        assert_eq!(f.len(), 10);
        assert_eq!(f32::from_le_bytes([f[6], f[7], f[8], f[9]]), 21.0);
    }

    #[test]
    fn acks_are_read() {
        let mut b = can_fly_body(1, 9);
        b.extend_from_slice(&0x0100_0000u32.to_le_bytes()); // flags: can fly
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&1234u32.to_le_bytes());
        for v in [16226.2f32, 16257.0, 13.25, 1.5] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0f32.to_le_bytes());
        assert_eq!(ack_counter(&b), Some(9));
        let p = ack_pos(&b).unwrap();
        assert!((p.x - 16226.2).abs() < 1e-3 && (p.z - 13.25).abs() < 1e-6 && (p.o - 1.5).abs() < 1e-6);
        assert_eq!(ack_pos(&b[..10]), None);
    }

    #[test]
    fn spell_bodies_from_a_creature() {
        let g = crate::companion::GUID;
        let s = spell_start_body(g, BUILD_SPELL, 3000, [1.0, 2.0, 3.0]);
        let mut gb = Vec::new();
        packed_guid(&mut gb, g);
        let n = gb.len();
        assert_eq!(&s[..n], &gb[..]);
        assert_eq!(&s[n..2 * n], &gb[..]);
        let at = 2 * n;
        assert_eq!(s[at], 0);
        assert_eq!(u32::from_le_bytes(s[at + 1..at + 5].try_into().unwrap()), BUILD_SPELL);
        assert_eq!(u32::from_le_bytes(s[at + 5..at + 9].try_into().unwrap()), CAST_START);
        assert_eq!(u32::from_le_bytes(s[at + 9..at + 13].try_into().unwrap()), 3000);
        assert_eq!(u32::from_le_bytes(s[at + 13..at + 17].try_into().unwrap()), TARGET_DEST);
        assert_eq!(s[at + 17], 0);
        assert_eq!(s.len(), at + 18 + 12);
        let go = spell_go_body(g, BUILD_SPELL, 99, [1.0, 2.0, 3.0]);
        assert_eq!(u32::from_le_bytes(go[at + 5..at + 9].try_into().unwrap()), CAST_GO);
        assert_eq!(&go[at + 13..at + 15], &[0, 0], "no units hit, none missed");
        assert_eq!(go.len(), at + 15 + 4 + 1 + 12 + 1);
        assert_eq!(*go.last().unwrap(), 0);
    }

    #[test]
    fn facing_body_turns_in_place() {
        let at = At { x: 1.0, y: 2.0, z: 3.0, o: 0.0 };
        let b = facing_body(crate::companion::GUID, at, 5, 1.25);
        let n = 1 + b[0].count_ones() as usize;
        assert_eq!(b[n], 0);
        assert_eq!(b[n + 1 + 12 + 4], 4, "facing angle");
        assert_eq!(f32::from_le_bytes(b[n + 18..n + 22].try_into().unwrap()), 1.25);
        assert_eq!(u32::from_le_bytes(b[n + 30..n + 34].try_into().unwrap()), 1, "one point");
        assert_eq!(b.len(), n + 34 + 12);
    }

    #[test]
    fn dummies_stand_in_the_ring_or_in_front_of_you() {
        let mut d = Dojo::default();
        let p = At { x: 100.0, y: 100.0, z: 13.2, o: 0.0 };
        let row = d.place_dummies(At { z: 40.0, ..p }, 3, 31144, 13.2);
        assert_eq!(row.len(), 3);
        assert!(row.iter().all(|x| (x.at.x - 105.0).abs() < 1e-4 && (x.at.o - PI).abs() < 1e-4 && x.at.z == 13.2), "on the ground, not where a flier is");
        assert!((row[0].at.y - 98.2).abs() < 1e-4 && (row[2].at.y - 101.8).abs() < 1e-4);
        assert_eq!(row[0].guid >> 48, 0xF130);
        assert_eq!((row[0].guid >> 24) & 0xFF_FFFF, 31144);
        assert_ne!(row[0].guid, row[1].guid);
        assert_ne!(row[0].guid, crate::companion::GUID);
        assert_eq!(d.clear_dummies().len(), 3);
        assert!(d.dummies.is_empty());
        // in a ring facing north (+y): the first dummy 3 yards north of its middle, facing south
        d.ring = Some(At { x: 0.0, y: 0.0, z: 13.2, o: PI / 2.0 });
        let r = d.place_dummies(p, 9, 31144, 13.2);
        assert_eq!(r.len(), MAX_DUMMIES);
        assert!(r[0].at.x.abs() < 1e-4 && (r[0].at.y - 3.0).abs() < 1e-4 && (r[0].at.o - 1.5 * PI).abs() < 1e-4);
        assert!(d.line().starts_with("server=lite;fly=0;speed=1;dummies=5;ring=1;building=0"));
        assert!(dummy(31144).is_some() && dummy(1).is_none());
        for (_, _, name, _) in DUMMIES {
            assert!(!crate::companion::branded(name));
        }
    }

    #[test]
    fn counters_climb_and_entering_resets_the_moves() {
        let mut d = Dojo { fly: true, speed: 4, ..Default::default() };
        assert_eq!(d.counter(), 1);
        assert_eq!(d.counter(), 2);
        d.entered();
        assert!(!d.fly && d.speed_now() == 1);
    }

    #[test]
    fn a_build_waits_for_her_walk_then_shows_a_piece_each_step() {
        let stand = At { x: 0.0, y: 0.0, z: 0.0, o: 0.0 };
        let origin = At { x: 5.0, y: 0.0, z: 0.0, o: 0.0 };
        let pieces = vec![obj(3, 9.0, 0.0), obj(1, 1.0, 0.0), obj(2, 0.0, 4.0)];
        let mut j = Job::new(stand, origin, pieces, 1000, 600, "Garden");
        // all fences (one tier): nearest her first, each a big piece's time
        assert_eq!(j.pieces.iter().map(|o| o.guid).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(j.paces, vec![400, 400, 400]);
        assert!(j.due(1500, |_| true).is_empty(), "still walking");
        let s = j.due(1600, |_| true);
        assert!(matches!(s[0], Step::Start { ms: 1450, .. }), "{s:?}");
        assert_eq!(s.len(), 1);
        let s = j.due(1850, |_| true);
        assert!(matches!(&s[0], Step::Piece { obj, facing } if obj.guid == 1 && facing.abs() < 1e-5));
        // a piece taken back meanwhile never appears; a late tick catches up
        let s = j.due(3100, |g| g != 2);
        assert_eq!(s.len(), 2);
        assert!(matches!(&s[0], Step::Piece { obj, .. } if obj.guid == 3));
        assert_eq!(s[1], Step::Done { shown: 2, admired: false });
        // a long walk is cut short
        let j2 = Job::new(stand, origin, vec![], 0, 60_000, "x");
        assert_eq!(j2.start_at, WALK_MAX_MS);
    }

    #[test]
    fn she_stands_between_you_and_the_build_facing_it() {
        let p = At { x: 0.0, y: 0.0, z: 1.0, o: 0.0 };
        let origin = ahead(p, 10.0);
        let s = stand_spot(p, origin);
        assert!((s.x - 5.0).abs() < 1e-4 && (s.y - 1.5).abs() < 1e-4 && (s.z - 1.0).abs() < 1e-6);
        assert!((s.o - facing(s, 10.0, 0.0)).abs() < 1e-6);
        let l = local(At { x: 0.0, y: 0.0, z: 0.0, o: PI / 2.0 }, 1.0, 0.0);
        assert!(l.x.abs() < 1e-5 && (l.y - 1.0).abs() < 1e-5);
    }

    fn piece(guid: u64, entry: u32, x: f32, y: f32, z: f32) -> WorldObject {
        let t = crate::den::template(entry).unwrap();
        WorldObject { guid, entry, display: t.display, kind: t.kind, scale: t.size, x, y, z, o: 0.0, pitch: 0.0, roll: 0.0 }
    }

    #[test]
    fn she_builds_big_first_small_last_then_steps_back_to_look() {
        let ground = 13.2;
        let stand = At { x: 0.0, y: 0.0, z: ground, o: 0.0 };
        let origin = At { x: 0.5, y: 0.0, z: ground, o: 0.0 };
        let pieces = vec![
            piece(1, 2562, 2.0, 0.0, ground + 0.95), // bread on the table
            piece(2, 181075, 2.0, 0.0, ground),      // the table
            piece(3, 184592, 6.0, 0.0, ground),      // a tent
            piece(4, 2413, 2.0, 1.3, ground),        // a chair
            piece(5, 181396, 2.0, 0.0, ground),      // a rug under it all
        ];
        let mut j = Job::new(stand, origin, pieces, 0, 0, "Camp");
        assert_eq!(j.pieces.iter().map(|o| o.guid).collect::<Vec<_>>(), vec![5, 3, 2, 4, 1], "rug, tent, table, chair, bread");
        assert!(j.paces[1] > j.paces[3] && j.paces[3] > j.paces[4], "a tent takes longer than a chair, a chair than bread: {:?}", j.paces);
        let you = At { x: -10.0, y: 0.0, z: ground, o: 0.0 };
        j.admire = Some(admire_spot(origin, &j.pieces, you));
        let total: u64 = j.paces.iter().sum();
        let mut seen = Vec::new();
        let mut admired = None;
        let mut done = None;
        let mut t = 0;
        while t < PIECE_MS + total + ADMIRE_MS + 1000 && done.is_none() {
            for s in j.due(t, |_| true) {
                match s {
                    Step::Piece { obj, facing: f } => {
                        assert!((f - facing(stand, obj.x, obj.y)).abs() < 1e-5, "she faces each piece as it appears");
                        seen.push(obj.guid)
                    }
                    Step::Admire { to } => admired = Some((t, to)),
                    Step::Done { shown, admired } => done = Some((t, shown, admired)),
                    Step::Start { .. } => {}
                }
            }
            t += 50;
        }
        assert_eq!(seen, vec![5, 3, 2, 4, 1]);
        let (at, to) = admired.expect("she steps back");
        assert!(at >= PIECE_MS + total - 50, "after the last piece's own time: {at}");
        assert!(to.x < 0.0, "on your side of it: {to:?}");
        assert!((to.o - facing(to, origin.x, origin.y)).abs() < 1e-4, "facing the build");
        let (dt, shown, admired) = done.expect("then it is done");
        assert_eq!((shown, admired), (5, true));
        assert!(dt >= at + ADMIRE_MS && dt < at + ADMIRE_MS + 100);
        // no admire spot: done with the last piece, as before
        let mut k = Job::new(stand, origin, vec![piece(9, 179969, 2.0, 0.0, ground)], 0, 0, "x");
        let s = k.due(10_000, |_| true);
        assert!(matches!(s.last(), Some(Step::Done { shown: 1, admired: false })), "{s:?}");
        // everything taken back while she built: nothing to look at
        let mut u = Job::new(stand, origin, vec![piece(9, 179969, 2.0, 0.0, ground)], 0, 0, "x");
        u.admire = Some(you);
        let s = u.due(10_000, |_| false);
        assert!(matches!(s.last(), Some(Step::Done { shown: 0, admired: false })), "{s:?}");
    }

    #[test]
    fn she_builds_from_a_free_spot_in_the_middle_of_every_stamp() {
        let dir = std::path::Path::new("no-such-folder");
        for (name, _) in crate::den::BUILTIN_STAMPS {
            let st = crate::den::read_stamp(dir, name).unwrap();
            let mut den = crate::den::Den::new(crate::den::Spot { half: 32.0, ..Default::default() });
            let (x, y, z) = (den.spot.x + 2.0, den.spot.y, den.spot.z);
            let ch = den.stamp(&st, x, y, z, 0.0);
            assert_eq!(ch.created.len(), st.props.len(), "{name}: all of it inside the Den");
            let (fx, fy) = crate::stamps::focus(&st);
            let origin = local(At { x, y, z, o: 0.0 }, fx as f32, fy as f32);
            let s = build_stand(origin, &ch.created);
            for o in &ch.created {
                let sz = crate::stamps::size_of(o);
                if matches!(sz.kind, crate::stamps::Kind::Floor | crate::stamps::Kind::Shell) || o.z > z + 1.8 {
                    continue;
                }
                let b = crate::stamps::Bx { x: o.x, y: o.y, hw: sz.w / 2.0, hd: sz.d / 2.0, o: o.o };
                assert!(!b.holds(s.x, s.y, 0.3), "{name}: she stands inside entry {}", o.entry);
            }
            let d = ((s.x - origin.x).powi(2) + (s.y - origin.y).powi(2)).sqrt();
            assert!(d < 4.0, "{name}: she builds from near its middle ({d} yards off)");
            let you = At { x: x - 15.0, y, z, o: 0.0 };
            let a = admire_spot(origin, &ch.created, you);
            assert!(a.x < s.x, "{name}: she steps back your way: {a:?} from {s:?}");
        }
    }

    #[test]
    fn walk_facing_body_ends_facing() {
        let from = At { x: 1.0, y: 2.0, z: 3.0, o: 0.0 };
        let to = At { x: 4.0, y: 5.0, z: 6.0, o: 0.0 };
        let b = walk_facing_body(crate::companion::GUID, from, to, 7, 1500, 2.5);
        let n = 1 + b[0].count_ones() as usize;
        assert_eq!(f32::from_le_bytes(b[n + 1..n + 5].try_into().unwrap()), 1.0, "from");
        assert_eq!(b[n + 1 + 12 + 4], 4, "facing angle");
        assert_eq!(f32::from_le_bytes(b[n + 18..n + 22].try_into().unwrap()), 2.5);
        assert_eq!(u32::from_le_bytes(b[n + 26..n + 30].try_into().unwrap()), 1500, "how long the walk takes");
        assert_eq!(f32::from_le_bytes(b[n + 34..n + 38].try_into().unwrap()), 4.0, "to");
        assert_eq!(facing_body(crate::companion::GUID, from, 7, 2.5), walk_facing_body(crate::companion::GUID, from, from, 7, 100, 2.5));
    }

    #[test]
    fn every_stamp_answers_to_its_other_names() {
        for (said, name) in [("build a tavern", "inn"), ("make me a little camp", "campfire"), ("build a reading nook", "nook"),
                             ("build a blacksmith's forge", "smithy"), ("Build us a shrine please", "shrine"), ("make a farm", "garden"),
                             ("build a pier", "dock"), ("build a cottage", "house"), ("make a bazaar", "market"), ("build a castle", "castle")] {
            assert_eq!(build_words(said).as_deref(), Some(name), "{said}");
    #[test]
    fn kinds_go_up_in_their_places_and_the_walker_walks_a_circle() {
        let mut d = Dojo::default();
        let p = At { x: 0.0, y: 0.0, z: 1.0, o: 0.0 };
        let (gone, pack) = d.place_kind(p, Kind::Pack, 13.2);
        assert!(gone.is_empty() && pack.len() == 5);
        assert!(pack.iter().all(|x| x.kind == Kind::Pack && x.entry == 900_001 && x.at.z == 13.2 && x.at.x >= 6.0));
        let (gone, again) = d.place_kind(p, Kind::Pack, 13.2);
        assert_eq!(gone.len(), 5, "the old pack comes down");
        assert_eq!(d.dummies.len(), 5);
        assert_ne!(again[0].guid, pack[0].guid);
        let (_, boss) = d.place_kind(p, Kind::Boss, 13.2);
        assert_eq!(boss.len(), 1);
        assert!((boss[0].at.x - 12.0).abs() < 1e-4);
        assert_eq!(kind_of(boss[0].entry), Kind::Boss);
        assert_eq!(dummy(900_005).map(|x| x.2), Some("Practice Boss"));
        assert_eq!(kind_of(31144), Kind::Target);
        let (_, m) = d.place_kind(p, Kind::Mover, 13.2);
        let home = m[0].at;
        let mut seen = Vec::new();
        for _ in 0..MOVER_POINTS {
            let s = d.mover_steps();
            assert_eq!(s.len(), 1);
            seen.push(s[0].2);
        }
        let mid = At { x: home.x - MOVER_RADIUS, ..home };
        assert!(seen.iter().all(|a| ((a.x - mid.x).powi(2) + (a.y - mid.y).powi(2)).sqrt() - MOVER_RADIUS < 1e-3));
        assert!((seen[MOVER_POINTS - 1].x - home.x).abs() < 1e-3, "round again to where it started");
        for (_, _, name, _, _) in KIND_DUMMIES {
            assert!(!crate::companion::branded(name));
        }
    }
}
