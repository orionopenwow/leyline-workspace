//! Sandbox combat (owner 3 Nov: "have training dummies be active, make all spells work"): the Dojo's dummies are
//! targets, the player's auto attack and spells hit them, and the numbers come back as the game shows a real fight.
//! Simple on purpose: no talents, procs, gear, resistances or misses; damage from the spell's own numbers in the
//! player's Spell.dbc (spells.rs) scaled by level, plus a level-based spell power / attack power. Nothing dies but
//! the Dojo's boss; nothing hurts the player unless a dummy is set to hit back.
//!
//! What the game sends and gets (3.3.5a, as AzerothCore's Spell.cpp / Unit.cpp write them):
//! * CMSG_SET_SELECTION (u64): the target. CMSG_ATTACKSWING (u64) -> SMSG_ATTACKSTART (u64 attacker, u64 victim),
//!   then every swing SMSG_ATTACKERSTATEUPDATE (hit info, packed attacker and victim, damage, overkill, one school
//!   part, victim state, two u32); CMSG_ATTACKSTOP -> SMSG_ATTACKSTOP (packed both, u32 0).
//! * CMSG_CAST_SPELL (u8 cast count, u32 spell, u8 flags, the targets: u32 mask, a packed guid for a unit, the
//!   transport and x y z for a place) -> SMSG_SPELL_START (packed caster twice, the count, the spell, flags, the cast
//!   time, the targets, the power left), after the cast time SMSG_SPELL_GO (... the time, the units hit, none missed,
//!   the targets, the power left, a 0 after a place), the logs (SMSG_SPELLNONMELEEDAMAGELOG, SMSG_SPELLHEALLOG,
//!   SMSG_PERIODICAURALOG), the health in SMSG_UPDATE_OBJECT values, SMSG_SPELL_COOLDOWN, SMSG_AURA_UPDATE for auras
//!   with their duration, MSG_CHANNEL_START / UPDATE for channelled spells. Refused: SMSG_CAST_FAILED (count, spell,
//!   the reason, 0). Moving stops a cast: SMSG_SPELL_FAILURE and SMSG_SPELL_FAILED_OTHER (packed caster, count,
//!   spell, the reason).
//!
//! Everything here is plain state and message bodies (tested below); world.rs sends what it returns.

use crate::classes::{self, Kit, Stats};
use crate::companion::At;
use crate::raw::packed_guid;
use crate::pets::{self, PetDef};
use crate::spells::{Book, Effect, Spell};
use std::collections::HashMap;

pub const SMSG_UPDATE_OBJECT: u16 = 0x0A9;
pub const CMSG_CAST_SPELL: u32 = 0x12E;
pub const CMSG_CANCEL_CAST: u32 = 0x12F;
pub const SMSG_CAST_FAILED: u16 = 0x130;
pub const SMSG_SPELL_START: u16 = 0x131;
pub const SMSG_SPELL_GO: u16 = 0x132;
pub const SMSG_SPELL_FAILURE: u16 = 0x133;
pub const SMSG_SPELL_COOLDOWN: u16 = 0x134;
pub const CMSG_CANCEL_AURA: u32 = 0x136;
pub const MSG_CHANNEL_START: u16 = 0x139;
pub const MSG_CHANNEL_UPDATE: u16 = 0x13A;
pub const CMSG_CANCEL_CHANNELLING: u32 = 0x13B;
pub const CMSG_SET_SELECTION: u32 = 0x13D;
pub const CMSG_ATTACKSWING: u32 = 0x141;
pub const CMSG_ATTACKSTOP: u32 = 0x142;
pub const SMSG_ATTACKSTART: u16 = 0x143;
pub const SMSG_ATTACKSTOP: u16 = 0x144;
pub const SMSG_ATTACKSWING_NOTINRANGE: u16 = 0x145;
pub const SMSG_ATTACKSWING_CANT_ATTACK: u16 = 0x149;
pub const SMSG_ATTACKERSTATEUPDATE: u16 = 0x14A;
pub const SMSG_SPELLHEALLOG: u16 = 0x150;
pub const SMSG_CLEAR_COOLDOWN: u16 = 0x1DE;
pub const SMSG_PERIODICAURALOG: u16 = 0x24E;
pub const SMSG_SPELLNONMELEEDAMAGELOG: u16 = 0x250;
pub const SMSG_SPELL_FAILED_OTHER: u16 = 0x2A6;
pub const SMSG_UPDATE_COMBO_POINTS: u16 = 0x39D;
pub const SMSG_AURA_UPDATE: u16 = 0x496;
pub const SMSG_SPELLLOGMISS: u16 = 0x24B;
pub const CMSG_CANCEL_MOUNT_AURA: u32 = 0x375;
/// UNIT_FIELD_MOUNTDISPLAYID; UNIT_FLAG_MOUNT with the player's own UNIT_FLAG_PLAYER_CONTROLLED.
pub const MOUNT_DISPLAY: u16 = 69;
pub const FLAGS_MOUNTED: u32 = 0x0800_0008;
pub const FLAGS_PLAYER: u32 = 0x0000_0008;

/// The game's messages read here (world.rs takes them before the message crate does).
pub const CLIENT_OPS: [u32; 9] = [
    CMSG_CAST_SPELL, CMSG_CANCEL_CAST, CMSG_CANCEL_AURA, CMSG_CANCEL_CHANNELLING, CMSG_SET_SELECTION, CMSG_ATTACKSWING, CMSG_ATTACKSTOP,
    classes::CMSG_SET_ACTION_BUTTON, CMSG_CANCEL_MOUNT_AURA,
];
/// Every message this module writes (the headless test client reads them by hand).
pub const SERVER_OPS: [u16; 23] = [
    SMSG_CAST_FAILED, SMSG_SPELL_START, SMSG_SPELL_GO, SMSG_SPELL_FAILURE, SMSG_SPELL_COOLDOWN, MSG_CHANNEL_START, MSG_CHANNEL_UPDATE,
    SMSG_ATTACKSTART, SMSG_ATTACKSTOP, SMSG_ATTACKSWING_NOTINRANGE, SMSG_ATTACKSWING_CANT_ATTACK, SMSG_ATTACKERSTATEUPDATE, SMSG_SPELLHEALLOG,
    SMSG_CLEAR_COOLDOWN, SMSG_PERIODICAURALOG, SMSG_SPELLNONMELEEDAMAGELOG, SMSG_SPELL_FAILED_OTHER, SMSG_UPDATE_COMBO_POINTS, SMSG_AURA_UPDATE,
    classes::SMSG_INITIAL_SPELLS, SMSG_SPELLLOGMISS, crate::specs::SMSG_TALENTS_INFO, crate::pets::SMSG_PET_SPELLS,
];

/// The update fields used (UpdateFields.h, 3.3.5a).
pub mod field {
    pub const TARGET: u16 = 18;
    pub const CHANNEL_OBJECT: u16 = 20;
    pub const CHANNEL_SPELL: u16 = 22;
    pub const BYTES_0: u16 = 23;
    pub const HEALTH: u16 = 24;
    pub const POWER1: u16 = 25;
    pub const MAXHEALTH: u16 = 32;
    pub const MAXPOWER1: u16 = 33;
    pub const LEVEL: u16 = 54;
    pub const FLAGS: u16 = 59;
    pub const BASEATTACKTIME: u16 = 62;
    pub const MINDAMAGE: u16 = 70;
    pub const MAXDAMAGE: u16 = 71;
    pub const DYNAMIC_FLAGS: u16 = 79;
    pub const BYTES_2: u16 = 122;
    pub const ATTACK_POWER: u16 = 123;
    /// PLAYER_VISIBLE_ITEM_1_ENTRYID; slot n at + 2n.
    pub const VISIBLE_ITEM: u16 = 283;
}

/// SpellCastResult (3.3.5a).
pub mod fail {
    pub const BAD_TARGETS: u8 = 12;
    pub const INTERRUPTED: u8 = 40;
    pub const MOVING: u8 = 51;
    pub const NOT_KNOWN: u8 = 63;
    pub const NOT_READY: u8 = 67;
    pub const NO_POWER: u8 = 85;
    pub const OUT_OF_RANGE: u8 = 97;
    pub const SPELL_IN_PROGRESS: u8 = 105;
    pub const TARGET_FRIENDLY: u8 = 115;
}

/// Target flags (SpellCastTargetFlags).
pub const TF_UNIT_MASK: u32 = 0x2 | 0x4 | 0x8 | 0x80 | 0x100 | 0x400 | 0x10000 | 0x100000;
pub const TF_OBJECT_MASK: u32 = TF_UNIT_MASK | 0x800 | 0x200 | 0x8000;
pub const TF_ITEM_MASK: u32 = 0x10 | 0x1000;
pub const TF_SOURCE: u32 = 0x20;
pub const TF_DEST: u32 = 0x40;
pub const TF_STRING: u32 = 0x2000;
pub const TF_UNIT: u32 = 0x2;

const CAST_FLAG_START: u32 = 0x2;
const CAST_FLAG_GO: u32 = 0x100;
const CAST_FLAG_POWER_LEFT: u32 = 0x800;

/// The global cooldown when a spell names none.
pub const GCD_MS: u64 = 1500;
/// Leeway for the game's own timing (it sends a cast as its cooldown ends).
pub const SLACK_MS: u64 = 250;
/// Melee reach, a ranged reach, and an area spell's radius when the table names none.
pub const MELEE: f32 = 6.0;
pub const RANGED: f32 = 40.0;
pub const AREA: f32 = 8.0;
/// A fight's numbers are said this long after the last hit.
pub const IDLE_MS: u64 = 5000;
/// The Dojo panel's readout at most once a second.
pub const READOUT_MS: u64 = 1000;
/// The boss's circle: every so often, how long to move out, how wide.
pub const BOSS_EVERY_MS: u64 = 9000;
pub const BOSS_WARN_MS: u32 = 2500;
pub const BOSS_RADIUS: f32 = 5.0;
/// The circle's look: a ground-targeted fire spell's own visuals (only its look; to see on the real game).
pub const BOSS_SPELL: u32 = 2120;
/// A dummy that hits back swings this often for this share of your health.
pub const HIT_BACK_MS: u64 = 2000;
/// The boss's cast to interrupt: every so often (the first a few seconds in), how long it casts, its look (a shadow
/// bolt's own: only its look; to see on the real game).
pub const BOSS_CAST_EVERY_MS: u64 = 15_000;
pub const BOSS_CAST_FIRST_MS: u64 = 1500;
pub const BOSS_CAST_MS: u32 = 3000;
pub const BOSS_CAST_SPELL: u32 = 686;
/// Heavy armour (the boss and the boss-level dummy): about two fifths off a level-80 player's physical hits.
pub const HEAVY_ARMOR: u32 = 10_643;
/// A proc fires at most this often (each passive's own).
pub const PROC_ICD_MS: u64 = 1000;
/// SpellMissInfo: miss, dodge.
pub const MISS: u8 = 1;
pub const DODGE: u8 = 3;

/// What kind of dummy a unit is (dojo.rs puts them up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// A training dummy: huge health, refilled.
    #[default]
    Target,
    /// One of a pack of five (area spells).
    Pack,
    /// One that walks a circle.
    Mover,
    /// A friendly one that keeps losing health: heal it.
    Healer,
    /// One that hits back (tank practice).
    Tank,
    /// The practice boss: dies, with timed circles to move out of and a cast to interrupt.
    Boss,
    /// The companion: friendly, never hurt.
    Friend,
    /// A dummy three levels below you, no armour (the low end of a level spread).
    Light,
    /// A boss-level dummy (three levels above you), heavy armour: misses, dodges and armour count.
    Heavy,
    /// The healing range: a group of five friends taking damage, one more than the others.
    Group,
    /// A distance marker on the range (friendly, nothing happens to it).
    Marker,
    /// Boss Rush's light version (bossrush.rs): a raid boss or add. It dies at 0 (never refilled), its health is set
    /// by the fight ([`Combat::set_health`]), and the fight does its swings, not this module.
    Raid,
}

impl Kind {
    pub fn hostile(self) -> bool {
        !matches!(self, Kind::Healer | Kind::Friend | Kind::Group | Kind::Marker)
    }
    /// Armour against physical damage.
    pub fn armor(self) -> u32 {
        match self {
            Kind::Heavy | Kind::Boss | Kind::Raid => HEAVY_ARMOR,
            _ => 0,
        }
    }
    /// FactionTemplate: 7 (neutral to all: yellow, attackable), 14 (hostile: the ones that hit back), 35 (friendly).
    pub fn faction(self) -> u32 {
        match self {
            Kind::Tank | Kind::Boss | Kind::Raid => 14,
            Kind::Healer | Kind::Friend | Kind::Group | Kind::Marker => 35,
            _ => 7,
        }
    }
    pub fn max_health(self, level: u32) -> u32 {
        match self {
            Kind::Target | Kind::Mover | Kind::Tank | Kind::Light | Kind::Heavy => 5_000_000,
            Kind::Pack => 1_000_000,
            Kind::Healer => 20_000 + 250 * level,
            Kind::Group => 15_000 + 200 * level,
            Kind::Boss => 1500 * level.max(1) + 30_000,
            Kind::Friend | Kind::Marker => 100,
            Kind::Raid => 1_000_000,
        }
    }
    /// Health when it goes up (the healing dummy starts hurt).
    pub fn start_health(self, level: u32) -> u32 {
        match self {
            Kind::Healer => self.max_health(level) * 3 / 10,
            Kind::Group => self.max_health(level) * 6 / 10,
            k => k.max_health(level),
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Kind::Target => "target",
            Kind::Pack => "pack",
            Kind::Mover => "mover",
            Kind::Healer => "healer",
            Kind::Tank => "tank",
            Kind::Boss => "boss",
            Kind::Friend => "friend",
            Kind::Light => "light",
            Kind::Heavy => "heavy",
            Kind::Group => "group",
            Kind::Marker => "range",
            Kind::Raid => "raid",
        }
    }
    pub fn from_word(w: &str) -> Option<Kind> {
        Some(match w {
            "target" | "targets" | "single" | "dummy" | "dummies" => Kind::Target,
            "pack" | "aoe" => Kind::Pack,
            "mover" | "moving" | "walker" => Kind::Mover,
            "healer" | "heal" | "healing" | "wounded" => Kind::Healer,
            "tank" | "defence" | "defense" | "hitter" => Kind::Tank,
            "boss" => Kind::Boss,
            "light" | "low" | "novice" => Kind::Light,
            "heavy" | "high" | "raider" | "armoured" | "armored" => Kind::Heavy,
            "group" | "healers" | "party" | "raid" | "healrange" | "healing range" => Kind::Group,
            "range" | "markers" | "distance" | "distances" => Kind::Marker,
            _ => return None,
        })
    }
}

/// A unit the player can fight or help, as world.rs knows it now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Unit {
    pub guid: u64,
    pub at: At,
    pub kind: Kind,
    pub level: u32,
}

/// What a call asks world.rs to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// A message to the player's game.
    Pkt(u16, Vec<u8>),
    /// A system line.
    Say(String),
    /// The middle of the screen (the boss's warnings).
    Middle(String),
    /// The events log.
    Event(&'static str, String),
    /// The Dojo panel's readout (`wuden dps <line>`).
    Dps(String),
    /// A timed parse is over (dojo.rs keeps the best per class).
    Parse { minutes: u32, dps: u32, total: u64, secs: f32 },
    /// The boss is down (the best time per class): the time, how often the circles caught you, the casts you
    /// interrupted of how many.
    Boss { secs: f32, hits: u32, kicked: u32, casts: u32 },
    /// The practice run is over (`.den dojo demo`): its numbers.
    Demo { secs: f32, total: u64, dps: u32 },
}

/// CMSG_CAST_SPELL as read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CastReq {
    pub count: u8,
    pub spell: u32,
    pub mask: u32,
    pub unit: Option<u64>,
    pub src: Option<[f32; 3]>,
    pub dest: Option<[f32; 3]>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(f32::from_bits)
}

/// A packed guid at `*at` (moves past it).
pub fn read_packed(b: &[u8], at: &mut usize) -> Option<u64> {
    let mask = *b.get(*at)?;
    *at += 1;
    let mut g = 0u64;
    for i in 0..8 {
        if mask & (1 << i) != 0 {
            g |= (*b.get(*at)? as u64) << (i * 8);
            *at += 1;
        }
    }
    Some(g)
}

fn read_place(b: &[u8], at: &mut usize) -> Option<[f32; 3]> {
    read_packed(b, at)?; // the transport
    let p = [f32_at(b, *at)?, f32_at(b, *at + 4)?, f32_at(b, *at + 8)?];
    *at += 12;
    p.iter().all(|v| v.is_finite()).then_some(p)
}

/// CMSG_CAST_SPELL's body.
pub fn read_cast(b: &[u8]) -> Option<CastReq> {
    let count = *b.first()?;
    let spell = u32_at(b, 1)?;
    let mask = u32_at(b, 6)?;
    let mut at = 10;
    let mut r = CastReq { count, spell, mask, ..Default::default() };
    if mask & TF_OBJECT_MASK != 0 {
        r.unit = Some(read_packed(b, &mut at)?);
    }
    if mask & TF_ITEM_MASK != 0 {
        read_packed(b, &mut at)?;
    }
    if mask & TF_SOURCE != 0 {
        r.src = Some(read_place(b, &mut at)?);
    }
    if mask & TF_DEST != 0 {
        r.dest = Some(read_place(b, &mut at)?);
    }
    Some(r)
}

/// A u64 at the start (CMSG_SET_SELECTION, CMSG_ATTACKSWING).
pub fn read_guid(b: &[u8]) -> Option<u64> {
    b.get(..8).map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

/// The targets as the server writes them (the mask, the unit, the places; never an item or a text).
fn write_targets(b: &mut Vec<u8>, mask: u32, unit: Option<u64>, src: Option<[f32; 3]>, dest: Option<[f32; 3]>) {
    let mask = mask & (TF_OBJECT_MASK | TF_SOURCE | TF_DEST);
    let mask = if unit.is_none() { mask & !TF_OBJECT_MASK } else { mask };
    let mask = if src.is_none() { mask & !TF_SOURCE } else { mask };
    let mask = if dest.is_none() { mask & !TF_DEST } else { mask };
    b.extend_from_slice(&mask.to_le_bytes());
    if mask & TF_OBJECT_MASK != 0 {
        packed_guid(b, unit.unwrap_or(0));
    }
    for (flag, p) in [(TF_SOURCE, src), (TF_DEST, dest)] {
        if mask & flag != 0 {
            let p = p.unwrap_or([0.0; 3]);
            b.push(0);
            for v in p {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
}

// ---------------------------------------------------------------- message bodies

/// SMSG_UPDATE_OBJECT with one values block: (field, value) pairs.
pub fn values_body(guid: u64, fields: &[(u16, u32)]) -> Vec<u8> {
    let mut f = fields.to_vec();
    f.sort_by_key(|x| x.0);
    f.dedup_by_key(|x| x.0);
    let max = f.last().map(|x| x.0).unwrap_or(0) as usize;
    let blocks = max / 32 + 1;
    let mut mask = vec![0u32; blocks];
    for (i, _) in &f {
        mask[*i as usize / 32] |= 1 << (i % 32);
    }
    let mut b = Vec::with_capacity(16 + blocks * 4 + f.len() * 4);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(0); // UPDATETYPE_VALUES
    packed_guid(&mut b, guid);
    b.push(blocks as u8);
    for m in mask {
        b.extend_from_slice(&m.to_le_bytes());
    }
    for (_, v) in f {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

pub fn attack_start_body(attacker: u64, victim: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(16);
    b.extend_from_slice(&attacker.to_le_bytes());
    b.extend_from_slice(&victim.to_le_bytes());
    b
}

pub fn attack_stop_body(attacker: u64, victim: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(22);
    packed_guid(&mut b, attacker);
    packed_guid(&mut b, victim);
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

/// HITINFO_AFFECTS_VICTIM | (crit: HITINFO_CRITICALHIT).
pub fn swing_body(attacker: u64, victim: u64, damage: u32, crit: bool) -> Vec<u8> {
    swing_outcome_body(attacker, victim, damage, 0x2 | if crit { 0x80 } else { 0 }, 1)
}

/// How a swing went (hit info, victim state): a hit, a critical hit, a glancing blow (HITINFO_GLANCING 0x4000), a
/// miss (HITINFO_MISS 0x10, no damage), a dodge (VICTIMSTATE_DODGE 2).
pub fn swing_outcome(crit: bool, glancing: bool, miss: bool, dodge: bool) -> (u32, u8) {
    if miss {
        (0x10, 0)
    } else if dodge {
        (0x0, 2)
    } else {
        (0x2 | if crit { 0x80 } else { 0 } | if glancing { 0x4000 } else { 0 }, 1)
    }
}

/// SMSG_ATTACKERSTATEUPDATE: hit info, packed attacker and victim, damage, overkill, one physical part (school,
/// float and whole damage, then the absorbed and resisted parts' absence), the victim's state, two u32.
pub fn swing_outcome_body(attacker: u64, victim: u64, damage: u32, hit: u32, state: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    b.extend_from_slice(&hit.to_le_bytes());
    packed_guid(&mut b, attacker);
    packed_guid(&mut b, victim);
    b.extend_from_slice(&damage.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // overkill
    b.push(1);
    b.extend_from_slice(&1u32.to_le_bytes()); // physical
    b.extend_from_slice(&(damage as f32).to_le_bytes());
    b.extend_from_slice(&damage.to_le_bytes());
    b.push(state); // VICTIMSTATE_HIT 1, DODGE 2, a miss 0
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

/// SMSG_SPELLLOGMISS (the combat log's miss line): spell, the caster's full guid, a 0, one target: its full guid
/// and why (SpellMissInfo: 1 miss, 3 dodge).
pub fn spell_miss_body(spell: u32, caster: u64, target: u64, why: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(22);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&caster.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&target.to_le_bytes());
    b.push(why);
    b
}

/// SMSG_SPELLNONMELEEDAMAGELOG: packed target and attacker, spell, damage, overkill, the school, absorbed, resisted,
/// periodic, unused, blocked, hit info (SPELL_HIT_TYPE_CRIT 0x2), a 0.
pub fn spell_damage_body(target: u64, attacker: u64, spell: u32, damage: u32, overkill: u32, school_mask: u32, crit: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, target);
    packed_guid(&mut b, attacker);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&damage.to_le_bytes());
    b.extend_from_slice(&overkill.to_le_bytes());
    b.push(school_mask.max(1) as u8);
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(0);
    b.push(0);
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&(if crit { 0x2u32 } else { 0 }).to_le_bytes());
    b.push(0);
    b
}

/// SMSG_SPELLHEALLOG: packed target and caster, spell, heal, overheal, absorbed, crit, a 0.
pub fn heal_body(target: u64, caster: u64, spell: u32, heal: u32, overheal: u32, crit: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(36);
    packed_guid(&mut b, target);
    packed_guid(&mut b, caster);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&heal.to_le_bytes());
    b.extend_from_slice(&overheal.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(crit as u8);
    b.push(0);
    b
}

/// SMSG_PERIODICAURALOG for one tick: damage (aura 3, 53, 89: amount, overkill, school, absorb, resist, crit) or a
/// heal (aura 8, 20: amount, overheal, absorb, crit).
pub fn periodic_body(target: u64, caster: u64, spell: u32, aura: u32, amount: u32, school_mask: u32, heal: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, target);
    packed_guid(&mut b, caster);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&aura.to_le_bytes());
    b.extend_from_slice(&amount.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    if !heal {
        b.extend_from_slice(&school_mask.max(1).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
    }
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(0);
    b
}

/// SMSG_SPELL_START from the player (its packed guid twice), with the power left.
pub fn start_body(caster: u64, r: &CastReq, cast_ms: u32, power: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, caster);
    packed_guid(&mut b, caster);
    b.push(r.count);
    b.extend_from_slice(&r.spell.to_le_bytes());
    b.extend_from_slice(&(CAST_FLAG_START | CAST_FLAG_POWER_LEFT).to_le_bytes());
    b.extend_from_slice(&cast_ms.to_le_bytes());
    write_targets(&mut b, r.mask, r.unit, r.src, r.dest);
    b.extend_from_slice(&power.to_le_bytes());
    b
}

/// SMSG_SPELL_GO from `caster`: the units hit (full guids), none missed, the targets, the power left (`power`:
/// Some for the player's own casts), a 0 after a place.
pub fn go_body(caster: u64, r: &CastReq, time_ms: u32, hits: &[u64], power: Option<u32>) -> Vec<u8> {
    go_body_missed(caster, r, time_ms, hits, &[], power)
}

/// ... with the units missed: each its full guid and why (SpellMissInfo).
pub fn go_body_missed(caster: u64, r: &CastReq, time_ms: u32, hits: &[u64], missed: &[(u64, u8)], power: Option<u32>) -> Vec<u8> {
    let mut b = Vec::with_capacity(64 + hits.len() * 8 + missed.len() * 9);
    packed_guid(&mut b, caster);
    packed_guid(&mut b, caster);
    b.push(r.count);
    b.extend_from_slice(&r.spell.to_le_bytes());
    let flags = CAST_FLAG_GO | if power.is_some() { CAST_FLAG_POWER_LEFT } else { 0 };
    b.extend_from_slice(&flags.to_le_bytes());
    b.extend_from_slice(&time_ms.to_le_bytes());
    let hits = &hits[..hits.len().min(255)];
    b.push(hits.len() as u8);
    for h in hits {
        b.extend_from_slice(&h.to_le_bytes());
    }
    let missed = &missed[..missed.len().min(255)];
    b.push(missed.len() as u8);
    for (g, why) in missed {
        b.extend_from_slice(&g.to_le_bytes());
        b.push(*why);
    }
    write_targets(&mut b, r.mask, r.unit, r.src, r.dest);
    if let Some(p) = power {
        b.extend_from_slice(&p.to_le_bytes());
    }
    if r.mask & TF_DEST != 0 && r.dest.is_some() {
        b.push(0);
    }
    b
}

/// SMSG_CAST_FAILED: the count, the spell, the reason, no more casts.
pub fn failed_body(count: u8, spell: u32, why: u8) -> Vec<u8> {
    let mut b = vec![count];
    b.extend_from_slice(&spell.to_le_bytes());
    b.push(why);
    b.push(0);
    b
}

/// SMSG_SPELL_FAILURE / SMSG_SPELL_FAILED_OTHER: packed caster, the count, the spell, the reason.
pub fn interrupted_body(caster: u64, count: u8, spell: u32, why: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(16);
    packed_guid(&mut b, caster);
    b.push(count);
    b.extend_from_slice(&spell.to_le_bytes());
    b.push(why);
    b
}

/// SMSG_SPELL_COOLDOWN: the caster (u64), flags 0, one spell and its cooldown.
pub fn cooldown_body(caster: u64, spell: u32, ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(17);
    b.extend_from_slice(&caster.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&ms.to_le_bytes());
    b
}

/// SMSG_CLEAR_COOLDOWN: the spell, the caster.
pub fn clear_cooldown_body(spell: u32, caster: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(12);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&caster.to_le_bytes());
    b
}

/// MSG_CHANNEL_START: packed caster, the spell, the duration. MSG_CHANNEL_UPDATE: packed caster, the time left.
pub fn channel_start_body(caster: u64, spell: u32, ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(16);
    packed_guid(&mut b, caster);
    b.extend_from_slice(&spell.to_le_bytes());
    b.extend_from_slice(&ms.to_le_bytes());
    b
}

pub fn channel_update_body(caster: u64, left_ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(12);
    packed_guid(&mut b, caster);
    b.extend_from_slice(&left_ms.to_le_bytes());
    b
}

#[allow(clippy::too_many_arguments)]
/// SMSG_AURA_UPDATE: the unit, the slot, the spell; then (when not a removal) the flags (the effects' bits, 0x08 when
/// the caster is the unit itself, 0x10 helpful or 0x80 harmful, 0x20 with a duration), the level, the stack, the
/// caster when it is another, the duration and the time left.
pub fn aura_body(unit: u64, slot: u8, spell: u32, caster: u64, effects: u8, helpful: bool, level: u8, duration: Option<(u32, u32)>) -> Vec<u8> {
    let mut b = Vec::with_capacity(32);
    packed_guid(&mut b, unit);
    b.push(slot);
    b.extend_from_slice(&spell.to_le_bytes());
    if spell == 0 {
        return b;
    }
    let mut flags = effects & 0x7;
    if caster == unit {
        flags |= 0x08;
    }
    flags |= if helpful { 0x10 } else { 0x80 };
    if duration.is_some() {
        flags |= 0x20;
    }
    b.push(flags);
    b.push(level);
    b.push(1);
    if caster != unit {
        packed_guid(&mut b, caster);
    }
    if let Some((max, left)) = duration {
        b.extend_from_slice(&max.to_le_bytes());
        b.extend_from_slice(&left.to_le_bytes());
    }
    b
}

pub fn combo_body(target: u64, points: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(10);
    packed_guid(&mut b, target);
    b.push(points);
    b
}

// ---------------------------------------------------------------- the formulas

/// SpellEffects that hurt (school damage, weapon damage of all kinds, health leech).
pub fn is_damage(e: &Effect) -> bool {
    matches!(e.kind, 2 | 9 | 17 | 31 | 58 | 121)
}
pub fn is_weapon(e: &Effect) -> bool {
    matches!(e.kind, 17 | 31 | 58 | 121)
}
pub fn is_heal(e: &Effect) -> bool {
    matches!(e.kind, 10 | 136 | 67)
}
/// Auras that hurt over time (periodic damage, leech, percent damage) and that heal over time.
pub fn is_dot(e: &Effect) -> bool {
    e.kind == 6 && matches!(e.aura, 3 | 53 | 89)
}
pub fn is_hot(e: &Effect) -> bool {
    matches!(e.kind, 6 | 35 | 65) && matches!(e.aura, 8 | 20)
}
/// Implicit targets aimed at an enemy (the unit, areas of enemies, cones, the channel's target).
pub fn enemy_target(t: u32) -> bool {
    matches!(t, 6 | 15 | 16 | 22 | 24 | 28 | 53 | 77 | 104 | 108)
}
/// Areas: around the caster (15, 22, 24, 104, 108) or around a place (16, 28, 53).
pub fn area_around_me(t: u32) -> bool {
    matches!(t, 15 | 22 | 24 | 104 | 108)
}
pub fn area_around_place(t: u32) -> bool {
    matches!(t, 16 | 28)
}

/// A spell is harmful when one of its effects hurts or aims at an enemy.
pub fn harmful(s: &Spell) -> bool {
    s.effects.iter().any(|e| e.kind != 0 && (is_damage(e) || is_dot(e) || enemy_target(e.target_a) || (e.kind == 6 && e.aura == 23 && enemy_target(e.target_a))))
}

/// An effect's value at a level (AzerothCore SpellEffectInfo::CalcValue): base points, plus the points per level
/// over the spell's level (the level held between its base and max level), plus 1..die sides.
pub fn effect_value(s: &Spell, e: &Effect, level: u32, roll: u32) -> i32 {
    let mut lvl = level as i32;
    if s.max_level > 0 && lvl > s.max_level as i32 {
        lvl = s.max_level as i32;
    } else if lvl < s.base_level as i32 {
        lvl = s.base_level as i32;
    }
    let over = (lvl - s.spell_level as i32).max(0);
    let base = e.base + (over as f32 * e.per_level) as i32;
    let die = if e.die > 0 { 1 + (roll % e.die as u32) as i32 } else { 0 };
    base + die
}

/// The share of spell power a direct spell gets: its cast time over 3.5 s (instants as 1.5 s), at most 1.
pub fn coefficient(cast_ms: u32) -> f32 {
    (cast_ms.max(1500) as f32 / 3500.0).min(1.0)
}

/// One weapon swing: the weapon's damage by level and the attack power over 14 per second of swing.
pub fn weapon_hit(st: &Stats, roll: u32) -> u32 {
    let spread = (st.max_dmg - st.min_dmg).max(0.0);
    let w = st.min_dmg + spread * (roll % 1000) as f32 / 1000.0;
    (w + st.attack_power as f32 / 14.0 * st.attack_ms as f32 / 1000.0).round().max(1.0) as u32
}

/// The share of a physical hit armour takes away (3.3.5a: armour / (armour + 400 + 85 level, + 4.5 x 85 per level
/// over 59)), at most three quarters.
pub fn armor_reduction(armor: u32, level: u32) -> f32 {
    if armor == 0 {
        return 0.0;
    }
    let l = level.max(1) as f32;
    let k = 400.0 + 85.0 * l + if l > 59.0 { 4.5 * 85.0 * (l - 59.0) } else { 0.0 };
    (armor as f32 / (armor as f32 + k)).min(0.75)
}

/// Against a target `diff` levels above you: the chance (per cent) to miss and to be dodged, less your hit. The
/// sandbox's simplification: no misses against your own level or lower (you're hit-capped against them); against
/// higher levels the game's own numbers (spells 5, 6, 17 per cent; weapons 5.5, 6, 8 and a 6.5 per cent dodge at +3).
/// Tier 7 gear and up is hit- and expertise-capped (hit 100).
pub fn miss_chances(diff: i32, spell: bool, hit: f32) -> (f32, f32) {
    if diff <= 0 {
        return (0.0, 0.0);
    }
    let d = diff.min(3) as usize;
    let (miss, dodge) = if spell { ([0.0, 5.0, 6.0, 17.0][d], 0.0) } else { ([0.0, 5.5, 6.0, 8.0][d], [0.0, 5.5, 6.0, 6.5][d]) };
    ((miss - hit).max(0.0), if hit >= 100.0 { 0.0 } else { dodge })
}

/// A spell that stops a cast (SPELL_EFFECT_INTERRUPT_CAST 68, a silence 27 or a stun 12): what a kick is.
pub fn interrupts(s: &Spell) -> bool {
    s.effects.iter().any(|e| e.kind == 68 || (e.kind == 6 && matches!(e.aura, 12 | 27)))
}

/// The numbers without a class chosen: the ready-made level-1 character (as its create: 100 health, rage 1000).
pub fn plain_stats() -> Stats {
    Stats { level: 1, health: 100, power: classes::RAGE, max_power: 1000, attack_ms: 2000, min_dmg: 2.0, max_dmg: 4.0, attack_power: 30, spell_power: 5 }
}

// ---------------------------------------------------------------- the state

#[derive(Debug, Clone, PartialEq)]
struct Casting {
    req: CastReq,
    spell: Spell,
    ends: u64,
    target: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
struct Channel {
    req: CastReq,
    spell: Spell,
    target: Option<u64>,
    ends: u64,
    every: u64,
    next: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct Aura {
    unit: u64,
    slot: u8,
    spell: u32,
    /// 0: until cancelled.
    ends: u64,
    every: u64,
    next: u64,
    /// Per tick (0: none), heal or damage, the aura type and school.
    amount: u32,
    heal: bool,
    aura: u32,
    school: u32,
    form: u8,
    /// A mount (its model), with the run and flight speeds it gives.
    mount: u32,
}

/// A unit's health.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hp {
    pub max: u32,
    pub cur: u32,
    pub kind: Kind,
    last_hit: u64,
    swing_next: u64,
}

/// A fight's numbers (damage, or healing on the healing dummy).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meter {
    pub first: u64,
    pub last: u64,
    pub total: u64,
    pub healing: bool,
    pub name: String,
    shown: u64,
}

impl Meter {
    pub fn secs(&self) -> f32 {
        ((self.last.saturating_sub(self.first)) as f32 / 1000.0).max(1.0)
    }
    pub fn per_sec(&self) -> u32 {
        (self.total as f32 / self.secs()).round() as u32
    }
    pub fn line(&self, done: bool) -> String {
        format!("dps={};total={};secs={:.1};dummy={}{}", self.per_sec(), self.total, self.secs(), self.name, if done { ";done=1" } else { "" })
    }
}

/// A timed parse: `minutes`, from the first hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Parse {
    pub minutes: u32,
    pub start: u64,
    pub total: u64,
}

/// The practice boss's fight.
#[derive(Debug, Clone, PartialEq)]
pub struct BossFight {
    pub guid: u64,
    pub start: u64,
    pub next_circle: u64,
    pub circle: Option<([f32; 3], u64)>,
    pub hits: u32,
    count: u8,
    /// Its cast to interrupt: the next, and the one going (when it lands, its count).
    pub next_cast: u64,
    pub casting: Option<(u64, u8)>,
    pub kicked: u32,
    pub casts: u32,
}

impl BossFight {
    fn new(guid: u64, now: u64) -> BossFight {
        BossFight { guid, start: now, next_circle: now + BOSS_EVERY_MS / 2, circle: None, hits: 0, count: 0, next_cast: now + BOSS_CAST_FIRST_MS, casting: None, kicked: 0, casts: 0 }
    }
}

/// Your pet (pets.rs): where it is, what it fights, its stance (react: 0 passive, 1 defensive, 2 aggressive) and
/// whether it stays put.
#[derive(Debug, Clone, PartialEq)]
pub struct Pet {
    pub guid: u64,
    pub def: PetDef,
    pub at: At,
    pub target: u64,
    pub next_hit: u64,
    pub next_move: u64,
    pub react: u8,
    pub stay: bool,
    move_id: u32,
}

/// A pet swings (or casts its bolt) this often, follows when this far, and reaches this far.
pub const PET_HIT_MS: u64 = 2000;
pub const PET_FOLLOW: f32 = 4.0;
pub const PET_REACH: f32 = 3.0;
pub const PET_BOLT_REACH: f32 = 30.0;

/// A practice run (`.den dojo demo`): Den Lite casts your main bar's rotation at a dummy for you, a timed parse.
#[derive(Debug, Clone, PartialEq)]
pub struct Demo {
    pub target: u64,
    pub start: u64,
    pub until: u64,
    pub total: u64,
    count: u8,
}

/// The player's fight (the host's: one player fights in a Den).
#[derive(Debug)]
pub struct Combat {
    pub me: u64,
    pub kit: Option<Kit>,
    pub stats: Stats,
    pub health: u32,
    pub power: f32,
    pub selection: u64,
    /// The unit swung at and when the next swing is due; ranged (Auto Shot) or not.
    pub swing: Option<(u64, u64, bool)>,
    casting: Option<Casting>,
    channel: Option<Channel>,
    pub gcd_until: u64,
    pub cooldowns: HashMap<u32, u64>,
    auras: Vec<Aura>,
    pub hp: HashMap<u64, Hp>,
    pub combo: (u64, u8),
    /// Dummies hit back (`.den dojo fight on`); the tank dummy always does.
    pub fight: bool,
    pub form: u8,
    pub meter: Meter,
    pub parse: Option<Parse>,
    pub boss: Option<BossFight>,
    rng: u64,
    next_regen: u64,
    last_hurt: u64,
    gos: u32,
    /// Spells known besides the class's (`.den mounts`: every mount).
    pub extra: Vec<u32>,
    speed_counter: u32,
    /// Critical chance, hit, bonuses and procs (the gear's tier, the talents' passives).
    pub mods: classes::Mods,
    /// Nothing hurts you (`.den dojo invulnerable`).
    pub god: bool,
    /// Healing done (the damage meter's twin).
    pub heals: Meter,
    /// The misses' own dice (the others' rolls stay as they were).
    rng2: u64,
    proc_ready: HashMap<u32, u64>,
    in_proc: bool,
    pub demo: Option<Demo>,
    pub pet: Option<Pet>,
    pets_made: u32,
    /// What you last attacked or cast at (your pet's target).
    pub last_target: u64,
}

impl Combat {
    pub fn new(me: u64) -> Combat {
        let st = plain_stats();
        Combat {
            me,
            kit: None,
            stats: st,
            health: st.health,
            power: st.max_power as f32,
            selection: 0,
            swing: None,
            casting: None,
            channel: None,
            gcd_until: 0,
            cooldowns: HashMap::new(),
            auras: Vec::new(),
            hp: HashMap::new(),
            combo: (0, 0),
            fight: false,
            form: 0,
            meter: Meter::default(),
            parse: None,
            boss: None,
            rng: 0x9E37_79B9_7F4A_7C15,
            next_regen: 0,
            last_hurt: 0,
            gos: 0,
            extra: Vec::new(),
            speed_counter: 0x4000_0000,
            mods: classes::Mods::default(),
            god: false,
            heals: Meter::default(),
            rng2: 0x2545_F491_4F6C_DD1D,
            proc_ready: HashMap::new(),
            in_proc: false,
            demo: None,
            pet: None,
            pets_made: 0,
            last_target: 0,
        }
    }

    /// The same numbers every run (the tests).
    pub fn seed(&mut self, s: u64) {
        self.rng = s.max(1);
    }

    fn roll(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x >> 32) as u32
    }

    /// A second die, for misses and procs (so a fight without them rolls as it always did).
    fn roll2(&mut self) -> u32 {
        let mut x = self.rng2;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng2 = x;
        (x >> 32) as u32
    }

    /// A chance in per cent, on the second die.
    fn chance(&mut self, pct: f32) -> bool {
        pct > 0.0 && ((self.roll2() % 10_000) as f32) < pct * 100.0
    }

    fn crit(&mut self) -> bool {
        self.roll() % 100 < self.mods.crit.round().clamp(0.0, 100.0) as u32
    }

    /// Does a hit on `g` miss (or get dodged)? Against higher levels only (miss_chances).
    fn missed(&mut self, units: &[Unit], g: u64, spell: bool) -> Option<u8> {
        let u = units.iter().find(|u| u.guid == g)?;
        if !u.kind.hostile() {
            return None;
        }
        let (miss, dodge) = miss_chances(u.level as i32 - self.stats.level as i32, spell, self.mods.hit);
        if self.chance(miss) {
            Some(MISS)
        } else if self.chance(dodge) {
            Some(DODGE)
        } else {
            None
        }
    }

    /// Damage after the bonuses and the target's armour (physical only).
    fn dealt(&self, units: &[Unit], g: u64, dmg: f32, school: u32) -> u32 {
        let armor = units.iter().find(|u| u.guid == g).map(|u| u.kind.armor()).unwrap_or(0);
        let phys = if school & 1 != 0 || school == 0 { 1.0 - armor_reduction(armor, self.stats.level) } else { 1.0 };
        (dmg * (1.0 + self.mods.dmg_pct / 100.0) * phys).max(1.0) as u32
    }

    /// A class chosen (or none): full health and power, nothing going on.
    pub fn set_kit(&mut self, kit: Option<Kit>) {
        self.stats = kit.as_ref().map(|k| k.stats).unwrap_or_else(plain_stats);
        self.mods = kit.as_ref().map(|k| k.mods.clone()).unwrap_or_default();
        self.kit = kit;
        self.entered();
    }

    /// A new time in the world: full, nothing cast, no auras.
    pub fn entered(&mut self) {
        self.health = self.stats.health;
        self.power = self.start_power();
        (self.swing, self.casting, self.channel) = (None, None, None);
        self.auras.clear();
        self.cooldowns.clear();
        self.gcd_until = 0;
        self.combo = (0, 0);
        self.form = 0;
        self.meter = Meter::default();
        self.heals = Meter::default();
        self.boss = None;
        self.parse = None;
        self.demo = None;
        self.pet = None;
        self.last_target = 0;
    }

    fn start_power(&self) -> f32 {
        self.stats.max_power as f32
    }

    pub fn power_field(&self) -> u16 {
        field::POWER1 + self.stats.power as u16
    }

    /// The player's health and power fields now.
    pub fn my_fields(&self) -> Vec<(u16, u32)> {
        vec![(field::HEALTH, self.health), (field::MAXHEALTH, self.stats.health), (self.power_field(), self.power as u32), (field::MAXPOWER1 + self.stats.power as u16, self.stats.max_power)]
    }

    fn me_update(&self) -> Out {
        Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::HEALTH, self.health), (self.power_field(), self.power as u32)]))
    }

    pub fn knows(&self, spell: u32) -> bool {
        self.extra.contains(&spell)
            || match &self.kit {
                Some(k) => k.spells.binary_search(&spell).is_ok(),
                None => spell == classes::ATTACK,
            }
    }

    /// Every spell known: the class's (or the auto attack) and the extra ones, sorted.
    pub fn known(&self) -> Vec<u32> {
        let mut v = self.kit.as_ref().map(|k| k.spells.clone()).unwrap_or_else(|| vec![classes::ATTACK]);
        v.extend(self.extra.iter().copied());
        v.sort_unstable();
        v.dedup();
        v
    }

    fn speed_packets(&mut self, run: f32, flight: f32) -> Vec<Out> {
        self.speed_counter = self.speed_counter.wrapping_add(2);
        vec![
            Out::Pkt(crate::dojo::SMSG_FORCE_RUN_SPEED_CHANGE, crate::dojo::run_speed_body(self.me, self.speed_counter, run)),
            Out::Pkt(crate::dojo::SMSG_FORCE_FLIGHT_SPEED_CHANGE, crate::dojo::flight_speed_body(self.me, self.speed_counter + 1, flight)),
        ]
    }

    /// CMSG_CANCEL_MOUNT_AURA: off the mount.
    pub fn dismount(&mut self) -> Vec<Out> {
        match self.auras.iter().position(|a| a.unit == self.me && a.mount != 0) {
            Some(i) => self.drop_aura(i),
            None => Vec::new(),
        }
    }

    pub fn mounted(&self) -> bool {
        self.auras.iter().any(|a| a.unit == self.me && a.mount != 0)
    }

    /// The health a unit goes up with (and Den Lite keeps from now).
    pub fn unit_health(&mut self, u: &Unit) -> Hp {
        *self.hp.entry(u.guid).or_insert(Hp { max: u.kind.max_health(u.level), cur: u.kind.start_health(u.level), kind: u.kind, last_hit: 0, swing_next: 0 })
    }

    /// A unit taken down.
    pub fn forget(&mut self, guid: u64) {
        self.hp.remove(&guid);
        self.auras.retain(|a| a.unit != guid);
        if self.swing.is_some_and(|s| s.0 == guid) {
            self.swing = None;
        }
        if self.boss.as_ref().is_some_and(|b| b.guid == guid) {
            self.boss = None;
        }
    }

    /// A unit's health set from outside (Boss Rush's fight owns its boss's and adds' health).
    pub fn set_health(&mut self, guid: u64, kind: Kind, max: u32, cur: u32) {
        let h = self.hp.entry(guid).or_insert(Hp { max, cur, kind, last_hit: 0, swing_next: 0 });
        (h.max, h.cur, h.kind) = (max.max(1), cur.min(max.max(1)), kind);
    }

    pub fn health_of(&self, guid: u64) -> Option<u32> {
        self.hp.get(&guid).map(|h| h.cur)
    }

    /// A hit from Boss Rush's fight: this one can take the player to 0 (the fight then ends in a wipe).
    pub fn raid_hit(&mut self, now: u64, dmg: u32) -> Vec<Out> {
        self.last_hurt = now;
        if self.god {
            return Vec::new();
        }
        let dmg = (dmg as f32 * (1.0 - armor_reduction(self.mods.armor, 83).min(0.6)) * (1.0 - self.mods.avoid / 100.0)) as u32;
        self.health = self.health.saturating_sub(dmg);
        vec![self.me_update()]
    }

    /// Healing from Boss Rush's companions.
    pub fn raid_heal(&mut self, amount: u32) -> Vec<Out> {
        if self.health == 0 {
            return Vec::new();
        }
        self.health = (self.health + amount).min(self.stats.health);
        vec![self.me_update()]
    }

    /// Full again (a new pull).
    pub fn refill_me(&mut self) -> Vec<Out> {
        self.health = self.stats.health;
        vec![self.me_update()]
    }

    pub fn select(&mut self, guid: u64) {
        self.selection = guid;
    }

    /// CMSG_ATTACKSWING: a unit to swing at (a hostile dummy), or why not.
    pub fn attack(&mut self, now: u64, units: &[Unit], guid: u64) -> Vec<Out> {
        let Some(u) = units.iter().find(|u| u.guid == guid) else {
            return vec![Out::Pkt(SMSG_ATTACKSWING_CANT_ATTACK, Vec::new())];
        };
        if !u.kind.hostile() {
            return vec![Out::Pkt(SMSG_ATTACKSWING_CANT_ATTACK, Vec::new())];
        }
        if self.hp.get(&guid).is_some_and(|h| h.cur == 0) {
            return vec![Out::Pkt(0x148, Vec::new())]; // SMSG_ATTACKSWING_DEADTARGET
        }
        self.unit_health(u);
        self.selection = guid;
        self.last_target = guid;
        self.swing = Some((guid, now, false));
        vec![Out::Pkt(SMSG_ATTACKSTART, attack_start_body(self.me, guid)), Out::Event("combat", format!("attack {}", u.kind.word()))]
    }

    /// CMSG_ATTACKSTOP.
    pub fn stop(&mut self) -> Vec<Out> {
        match self.swing.take() {
            Some((g, _, _)) => vec![Out::Pkt(SMSG_ATTACKSTOP, attack_stop_body(self.me, g))],
            None => Vec::new(),
        }
    }

    fn fail(&self, r: &CastReq, why: u8) -> Vec<Out> {
        vec![Out::Pkt(SMSG_CAST_FAILED, failed_body(r.count, r.spell, why))]
    }

    fn cost(&self, s: &Spell) -> f32 {
        // a power other than the class's (health, runes): free in the sandbox
        if s.power_type != self.stats.power as i32 {
            return 0.0;
        }
        if s.cost_pct > 0 {
            return self.stats.max_power as f32 * s.cost_pct as f32 / 100.0;
        }
        (s.cost + s.cost_per_level * self.stats.level) as f32
    }

    /// CMSG_CAST_SPELL.
    pub fn cast(&mut self, now: u64, me: At, units: &[Unit], book: &Book, r: CastReq) -> Vec<Out> {
        if !self.knows(r.spell) {
            return self.fail(&r, fail::NOT_KNOWN);
        }
        // the auto attacks are their own thing
        if r.spell == classes::ATTACK {
            let g = r.unit.unwrap_or(self.selection);
            return self.attack(now, units, g);
        }
        let Some(s) = book.spell(r.spell).cloned() else { return self.fail(&r, fail::NOT_KNOWN) };
        if r.spell == classes::AUTO_SHOT {
            let g = r.unit.unwrap_or(self.selection);
            let mut out = self.attack(now, units, g);
            if let Some(sw) = self.swing.as_mut() {
                sw.2 = true;
            }
            out.retain(|o| !matches!(o, Out::Pkt(SMSG_ATTACKSTART, _)));
            return out;
        }
        if self.casting.is_some() || self.channel.is_some() {
            return self.fail(&r, fail::SPELL_IN_PROGRESS);
        }
        if s.gcd > 0 && now + SLACK_MS < self.gcd_until {
            return self.fail(&r, fail::NOT_READY);
        }
        if self.cooldowns.get(&r.spell).is_some_and(|&t| now + SLACK_MS < t) {
            return self.fail(&r, fail::NOT_READY);
        }
        let cost = self.cost(&s);
        if cost > self.power + 0.5 {
            return self.fail(&r, fail::NO_POWER);
        }
        // the target: a hostile dummy for a harmful spell (or a place / an area around you), else you or a friend
        let target = r.unit.filter(|g| *g != 0);
        if harmful(&s) {
            let area = s.effects.iter().any(|e| area_around_me(e.target_a) || area_around_place(e.target_a)) || r.dest.is_some();
            match target.and_then(|g| units.iter().find(|u| u.guid == g)) {
                Some(u) if !u.kind.hostile() => return self.fail(&r, fail::TARGET_FRIENDLY),
                Some(u) => {
                    let range = if s.dmg_class == 2 || s.effects.iter().any(is_weapon) { MELEE } else { RANGED };
                    if dist(me, u.at) > range + 2.0 {
                        return self.fail(&r, fail::OUT_OF_RANGE);
                    }
                    if self.hp.get(&u.guid).is_some_and(|h| h.cur == 0) {
                        return self.fail(&r, fail::BAD_TARGETS);
                    }
                }
                None if area => {}
                None => return self.fail(&r, fail::BAD_TARGETS),
            }
        }
        let level = self.stats.level;
        let cast_ms = book.cast_ms(&s, level);
        if s.gcd > 0 {
            self.gcd_until = now + s.gcd as u64;
        }
        let mut out = vec![Out::Pkt(SMSG_SPELL_START, start_body(self.me, &r, cast_ms, (self.power - if cast_ms == 0 { cost } else { 0.0 }).max(0.0) as u32))];
        if cast_ms == 0 {
            out.extend(self.go(now, me, units, book, r, s, target));
        } else {
            self.casting = Some(Casting { req: r, spell: s, ends: now + cast_ms as u64, target });
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    /// A cast ends: SPELL_GO, the power, the cooldown, then what it does.
    fn go(&mut self, now: u64, me: At, units: &[Unit], book: &Book, r: CastReq, s: Spell, target: Option<u64>) -> Vec<Out> {
        let level = self.stats.level;
        let cost = self.cost(&s);
        self.power = (self.power - cost).max(0.0);
        let mut out = Vec::new();
        // who it touches: per effect, then all of them for the go's hit list
        let mut per: Vec<Vec<u64>> = Vec::new();
        for e in &s.effects {
            per.push(if e.kind == 0 { Vec::new() } else { self.touched(&s, e, me, units, book, &r, target) });
        }
        let mut hits: Vec<u64> = per.iter().flatten().copied().collect();
        hits.sort_unstable();
        hits.dedup();
        // a harmful spell can miss (or be dodged) against a higher level: those units take none of it
        let mut missed: Vec<(u64, u8)> = Vec::new();
        if harmful(&s) {
            let weaponish = s.dmg_class == 2 || s.dmg_class == 3 || s.effects.iter().any(is_weapon);
            for &g in &hits {
                if let Some(why) = self.missed(units, g, !weaponish) {
                    missed.push((g, why));
                }
            }
            if !missed.is_empty() {
                hits.retain(|g| !missed.iter().any(|m| m.0 == *g));
                for p in per.iter_mut() {
                    p.retain(|g| !missed.iter().any(|m| m.0 == *g));
                }
            }
        }
        self.gos = self.gos.wrapping_add(1);
        out.push(Out::Pkt(SMSG_SPELL_GO, go_body_missed(self.me, &r, now as u32, &hits, &missed, Some(self.power as u32))));
        for &(g, why) in &missed {
            out.push(Out::Pkt(SMSG_SPELLLOGMISS, spell_miss_body(s.id, self.me, g, why)));
        }
        if let Some(g) = target.filter(|g| harmful(&s) && units.iter().any(|u| u.guid == *g && u.kind.hostile())) {
            self.last_target = g;
        }
        // a pet: summoned, or sent away
        if let Some(entry) = self.summoned(&s) {
            out.extend(self.summon(now, me, entry));
        } else if s.name == "Dismiss Pet" || s.effects.iter().any(|e| e.kind == 102) {
            out.extend(self.dismiss());
        }
        // runic power from a rune strike (the sandbox's: ten a strike)
        if self.stats.power == classes::RUNIC && s.rune_cost != 0 && s.power_type != classes::RUNIC as i32 {
            self.power = (self.power + 100.0).min(self.stats.max_power as f32);
        }
        // a kick on the boss while it casts: its cast stops
        if interrupts(&s) {
            if let (Some(g), Some(b)) = (target, self.boss.as_ref()) {
                if b.guid == g && b.casting.is_some() && hits.contains(&g) {
                    out.extend(self.boss_kicked(now));
                }
            }
        }
        if s.recovery > 0 || s.category_recovery > 0 {
            let cd = s.recovery.max(s.category_recovery);
            self.cooldowns.insert(s.id, now + cd as u64);
            out.push(Out::Pkt(SMSG_SPELL_COOLDOWN, cooldown_body(self.me, s.id, cd)));
        }
        out.push(Out::Event("cast", format!("{} {}", s.id, s.name)));
        // a channel: its ticks come from the tick
        if s.channeled() {
            let dur = book.duration_ms(&s, level).min(60_000).max(1000);
            let every = s.effects.iter().map(|e| e.amplitude).find(|&a| a > 0).unwrap_or(1000).max(200) as u64;
            out.push(Out::Pkt(MSG_CHANNEL_START, channel_start_body(self.me, s.id, dur)));
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::CHANNEL_OBJECT, target.unwrap_or(0) as u32), (field::CHANNEL_OBJECT + 1, (target.unwrap_or(0) >> 32) as u32), (field::CHANNEL_SPELL, s.id)])));
            self.channel = Some(Channel { req: r, spell: s.clone(), target, ends: now + dur as u64, every, next: now + every });
        }
        let finisher = s.attributes_ex & (0x0010_0000 | 0x0040_0000) != 0;
        let combo = if finisher && self.combo.0 == target.unwrap_or(0) { self.combo.1.max(1) } else { 1 };
        let mut weapon_pct = 100i32;
        for e in s.effects.iter() {
            if e.kind == 31 {
                weapon_pct = effect_value(&s, e, level, 0).max(1);
            }
        }
        let cast_ms = book.cast_ms(&s, level);
        // the aura effects per unit: one aura for the spell on each (its effects' bits), after the loop
        let mut auras: Vec<(u64, Vec<(usize, Effect, i32)>)> = Vec::new();
        for (k, e) in s.effects.iter().enumerate() {
            let roll = self.roll();
            let value = effect_value(&s, e, level, roll);
            for &g in &per[k] {
                if is_damage(e) && !s.channeled() {
                    let mut dmg = if is_weapon(e) {
                        let wr = self.roll();
                        let w = weapon_hit(&self.stats, wr);
                        (w as i64 * weapon_pct as i64 / 100) as i32 + if e.kind == 31 { 0 } else { value }
                    } else {
                        value + (self.stats.spell_power as f32 * coefficient(cast_ms)) as i32
                    };
                    if finisher {
                        dmg *= combo as i32;
                    }
                    let crit = self.crit();
                    let dmg = self.dealt(units, g, dmg.max(1) as f32 * if crit { 1.5 } else { 1.0 }, s.school_mask);
                    out.extend(self.hurt(now, units, g, s.id, dmg, s.school_mask, crit, false));
                    out.extend(self.procs(now, me, units, book, g));
                } else if is_heal(e) {
                    let amount = match e.kind {
                        136 => self.max_of(g) * value.max(0) as u32 / 100,
                        67 => self.max_of(g),
                        _ => (value + (self.stats.spell_power as f32 * coefficient(cast_ms) * 1.88) as i32).max(1) as u32,
                    };
                    let crit = self.crit();
                    let amount = if crit { amount * 3 / 2 } else { amount };
                    let amount = (amount as f32 * (1.0 + self.mods.heal_pct / 100.0)) as u32;
                    out.extend(self.heal(now, g, s.id, amount, crit, None));
                } else if e.kind == 30 {
                    // energize: power back
                    self.power = (self.power + value.max(0) as f32).min(self.stats.max_power as f32);
                    out.push(self.me_update());
                } else if e.kind == 80 {
                    let had = if self.combo.0 == g { self.combo.1 } else { 0 };
                    self.combo = (g, had.saturating_add(value.clamp(1, 5) as u8).min(5));
                    out.push(Out::Pkt(SMSG_UPDATE_COMBO_POINTS, combo_body(g, self.combo.1)));
                } else if matches!(e.kind, 6 | 35 | 65 | 119 | 128 | 129 | 143) && !s.channeled() {
                    match auras.iter_mut().find(|x| x.0 == g) {
                        Some(x) => x.1.push((k, *e, value)),
                        None => auras.push((g, vec![(k, *e, value)])),
                    }
                }
            }
        }
        for (g, effs) in auras {
            out.extend(self.aura_on(now, book, &s, &effs, g, cast_ms));
        }
        if finisher && self.combo.1 > 0 {
            let t = self.combo.0;
            self.combo = (0, 0);
            out.push(Out::Pkt(SMSG_UPDATE_COMBO_POINTS, combo_body(t, 0)));
        }
        out.push(self.me_update());
        out
    }

    fn max_of(&self, g: u64) -> u32 {
        if g == self.me { self.stats.health } else { self.hp.get(&g).map(|h| h.max).unwrap_or(100) }
    }

    #[allow(clippy::too_many_arguments)]
    /// Who an effect touches: you, your target, or every hostile dummy in its area.
    fn touched(&self, s: &Spell, e: &Effect, me: At, units: &[Unit], book: &Book, r: &CastReq, target: Option<u64>) -> Vec<u64> {
        let radius = { let x = book.radius(e); if x > 0.0 { x } else { AREA } };
        let hostile_near = |at: [f32; 3], rad: f32| -> Vec<u64> {
            units.iter().filter(|u| u.kind.hostile() && self.hp.get(&u.guid).map_or(true, |h| h.cur > 0) && ((u.at.x - at[0]).powi(2) + (u.at.y - at[1]).powi(2)).sqrt() <= rad + 0.5).map(|u| u.guid).collect()
        };
        let hostile = |g: u64| units.iter().any(|u| u.guid == g && u.kind.hostile());
        let friendly = |g: u64| g == self.me || units.iter().any(|u| u.guid == g && !u.kind.hostile());
        if area_around_me(e.target_a) {
            return hostile_near([me.x, me.y, me.z], radius);
        }
        if area_around_place(e.target_a) || (e.target_a == 0 && r.dest.is_some()) || e.target_b == 16 || e.target_b == 28 {
            let at = r.dest.or(target.and_then(|g| units.iter().find(|u| u.guid == g).map(|u| [u.at.x, u.at.y, u.at.z]))).unwrap_or([me.x, me.y, me.z]);
            return hostile_near(at, radius);
        }
        if enemy_target(e.target_a) || (is_damage(e) && e.target_a == 0) {
            return target.filter(|g| hostile(*g)).into_iter().collect();
        }
        if harmful(s) && e.target_a == 0 {
            return target.filter(|g| hostile(*g)).into_iter().collect();
        }
        // you, or the friend you have targeted (a heal on the healing dummy)
        match target {
            Some(g) if matches!(e.target_a, 21 | 25 | 57 | 45 | 0) && friendly(g) => vec![g],
            _ => vec![self.me],
        }
    }

    /// Damage to a unit: the log, its health; the meter; the boss's fall, a dummy's refill.
    #[allow(clippy::too_many_arguments)]
    fn hurt(&mut self, now: u64, units: &[Unit], g: u64, spell: u32, dmg: u32, school: u32, crit: bool, periodic_aura: bool) -> Vec<Out> {
        let mut out = Vec::new();
        let Some(u) = units.iter().find(|u| u.guid == g).copied() else { return out };
        if !u.kind.hostile() {
            return out;
        }
        let hp = self.unit_health(&u);
        if hp.cur == 0 {
            return out;
        }
        let dealt = dmg.min(hp.cur);
        let overkill = dmg - dealt;
        if !periodic_aura {
            out.push(Out::Pkt(SMSG_SPELLNONMELEEDAMAGELOG, spell_damage_body(g, self.me, spell, dmg, overkill, school, crit)));
        }
        out.extend(self.took(now, &u, dealt as i64));
        out
    }

    /// A unit's health changed by `-delta` from the player (damage, or healing when negative): the field, the
    /// meter, a refill, the boss.
    fn took(&mut self, now: u64, u: &Unit, delta: i64) -> Vec<Out> {
        let mut out = Vec::new();
        let boss = u.kind == Kind::Boss;
        let Some(h) = self.hp.get_mut(&u.guid) else { return out };
        h.cur = (h.cur as i64 - delta).clamp(0, h.max as i64) as u32;
        h.last_hit = now;
        if h.swing_next == 0 {
            h.swing_next = now + HIT_BACK_MS;
        }
        // a training dummy never falls: refilled at a tenth (a tale counts it as brought down: taleplay.rs)
        let mut down = false;
        if !boss && h.kind != Kind::Raid && h.kind.hostile() && h.cur < h.max / 10 {
            h.cur = h.max;
            down = true;
        }
        let (cur, max) = (h.cur, h.max);
        if down {
            out.push(Out::Event("dummy_down", u.kind.word().to_string()));
        }
        out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, cur)])));
        let healing = delta < 0;
        let amount = delta.unsigned_abs();
        out.extend(self.count(now, healing, amount, dummy_name(u.kind)));
        if let Some(d) = self.demo.as_mut() {
            d.total += amount;
        }
        if let Some(p) = self.parse.as_mut() {
            if p.start == 0 {
                p.start = now;
                out.push(Out::Say(format!("Parse started: {} minute{}. Go!", p.minutes, if p.minutes == 1 { "" } else { "s" })));
            }
            p.total += amount;
        }
        if boss {
            if self.boss.as_ref().map(|b| b.guid) != Some(u.guid) {
                self.boss = Some(BossFight::new(u.guid, now));
                out.push(Out::Middle("The boss turns to you. Watch the ground, and interrupt its cast!".into()));
            }
            if cur == 0 {
                let b = self.boss.take().unwrap_or(BossFight::new(u.guid, now));
                if b.casting.is_some() {
                    out.push(Out::Pkt(SMSG_SPELL_FAILED_OTHER, interrupted_body(b.guid, b.count, BOSS_CAST_SPELL, fail::INTERRUPTED)));
                }
                let secs = (now.saturating_sub(b.start)) as f32 / 1000.0;
                if self.swing.is_some_and(|s| s.0 == u.guid) {
                    out.extend(self.stop());
                }
                // UNIT_DYNFLAG_DEAD (0x20): the game shows it fallen
                out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, 0), (field::DYNAMIC_FLAGS, 0x20)])));
                out.push(Out::Boss { secs, hits: b.hits, kicked: b.kicked, casts: b.casts });
                out.push(Out::Event("dojo", format!("boss down {secs:.1}s {} hits", b.hits)));
            }
        }
        let _ = max;
        out
    }

    /// Healing on you or a friendly unit: the log and the health.
    fn heal(&mut self, now: u64, g: u64, spell: u32, amount: u32, crit: bool, periodic: Option<u32>) -> Vec<Out> {
        let mut out = Vec::new();
        let (cur, max) = if g == self.me { (self.health, self.stats.health) } else {
            match self.hp.get(&g) { Some(h) => (h.cur, h.max), None => (99, 100) }
        };
        let done = amount.min(max - cur.min(max));
        let over = amount - done;
        match periodic {
            Some(aura) => out.push(Out::Pkt(SMSG_PERIODICAURALOG, periodic_body(g, self.me, spell, aura, amount, 0, true))),
            None => out.push(Out::Pkt(SMSG_SPELLHEALLOG, heal_body(g, self.me, spell, amount, over, crit))),
        }
        if g == self.me {
            self.health = (self.health + done).min(self.stats.health);
            out.push(self.me_update());
            if done > 0 {
                out.extend(self.count(now, true, done as u64, "yourself"));
            }
        } else if let Some(h) = self.hp.get(&g).copied() {
            let u = Unit { guid: g, at: At::default(), kind: h.kind, level: 1 };
            if done > 0 {
                out.extend(self.took(now, &u, -(done as i64)));
            }
        }
        out
    }

    /// Damage or healing done, on its meter; the Dojo panel's readout at most once a second.
    fn count(&mut self, now: u64, healing: bool, amount: u64, name: &str) -> Vec<Out> {
        let m = if healing { &mut self.heals } else { &mut self.meter };
        if m.total == 0 {
            *m = Meter { first: now, last: now, total: 0, healing, name: name.into(), shown: 0 };
        }
        m.total += amount;
        m.last = now;
        let shown = self.meter.shown.max(self.heals.shown);
        if now >= shown + READOUT_MS {
            self.meter.shown = now;
            self.heals.shown = now;
            return vec![Out::Dps(self.readout(false))];
        }
        Vec::new()
    }

    /// The Dojo panel's line: `dps=..;total=..;secs=..;dummy=..;hps=..;healed=..[;done=1]`.
    pub fn readout(&self, done: bool) -> String {
        let (d, h) = (&self.meter, &self.heals);
        let name = if d.total > 0 || h.total == 0 { &d.name } else { &h.name };
        let secs = if d.total > 0 { d.secs() } else { h.secs() };
        format!(
            "dps={};total={};secs={:.1};dummy={};hps={};healed={}{}",
            if d.total > 0 { d.per_sec() } else { 0 },
            d.total,
            secs,
            name,
            if h.total > 0 { h.per_sec() } else { 0 },
            h.total,
            if done { ";done=1" } else { "" }
        )
    }

    /// `.den dojo meter reset`: both meters cleared.
    pub fn reset_meters(&mut self) -> Vec<Out> {
        self.meter = Meter::default();
        self.heals = Meter::default();
        vec![Out::Dps("dps=0;total=0;secs=0;dummy=-;hps=0;healed=0".into())]
    }

    /// A direct hit on `g`: the passives' procs (each its chance, at most once a second each), cast for you at it
    /// (their own SPELL_GO, so their look shows; never a proc from a proc).
    fn procs(&mut self, now: u64, me: At, units: &[Unit], book: &Book, g: u64) -> Vec<Out> {
        let mut out = Vec::new();
        if self.in_proc || self.mods.procs.is_empty() {
            return out;
        }
        for (_, trigger, chance) in self.mods.procs.clone() {
            if self.proc_ready.get(&trigger).is_some_and(|t| now < *t) || !self.chance(chance as f32) {
                continue;
            }
            let Some(t) = book.spell(trigger).cloned() else { continue };
            self.proc_ready.insert(trigger, now + PROC_ICD_MS);
            self.in_proc = true;
            let target = if harmful(&t) { Some(g) } else { Some(self.me) };
            let r = CastReq { count: 0, spell: trigger, mask: TF_UNIT, unit: target, src: None, dest: None };
            out.extend(self.go(now, me, units, book, r, t, target));
            self.in_proc = false;
            out.push(Out::Event("proc", format!("{trigger}")));
        }
        out
    }

    /// The boss's cast was interrupted.
    fn boss_kicked(&mut self, _now: u64) -> Vec<Out> {
        let Some(b) = self.boss.as_mut() else { return Vec::new() };
        let Some((_, count)) = b.casting.take() else { return Vec::new() };
        b.kicked += 1;
        let guid = b.guid;
        vec![
            Out::Pkt(SMSG_SPELL_FAILURE, interrupted_body(guid, count, BOSS_CAST_SPELL, fail::INTERRUPTED)),
            Out::Pkt(SMSG_SPELL_FAILED_OTHER, interrupted_body(guid, count, BOSS_CAST_SPELL, fail::INTERRUPTED)),
            Out::Middle("Interrupted!".into()),
            Out::Event("dojo", "boss kicked".into()),
        ]
    }

    fn free_slot(&self, unit: u64) -> u8 {
        (0u8..56).find(|s| !self.auras.iter().any(|a| a.unit == unit && a.slot == *s)).unwrap_or(55)
    }

    /// An aura: shown on the unit with its duration (one for the spell, its aura effects' bits); damage or healing
    /// over time ticks from the tick; a stance or form sets the shapeshift byte (UNIT_FIELD_BYTES_2's fourth: the
    /// stance bar's own); a mount (SPELL_AURA_MOUNTED 78: a creature entry, its model from creatures.rs) sets
    /// UNIT_FIELD_MOUNTDISPLAYID and UNIT_FLAG_MOUNT, its speeds (auras 32 and 207: per cent more) the run and flight
    /// speeds.
    fn aura_on(&mut self, now: u64, book: &Book, s: &Spell, effs: &[(usize, Effect, i32)], g: u64, cast_ms: u32) -> Vec<Out> {
        let mut out = Vec::new();
        let dur = book.duration_ms(s, self.stats.level);
        let ends = if dur == 0 || dur == u32::MAX { 0 } else { now + dur as u64 };
        let mask: u8 = effs.iter().fold(0, |m, (k, _, _)| m | (1u8 << k));
        let form = effs.iter().find(|x| x.1.aura == 36).map(|x| x.1.misc.clamp(0, 255) as u8).unwrap_or(0);
        let mount = effs.iter().find(|x| x.1.aura == 78).map(|x| crate::creatures::display(x.1.misc.max(0) as u32).unwrap_or(0).max(1)).unwrap_or(0);
        // the same spell again on the same unit: refreshed in its slot (a stance or a mount: switched off)
        if let Some(i) = self.auras.iter().position(|a| a.unit == g && a.spell == s.id) {
            if (form != 0 || mount != 0) && (ends == 0 || mount != 0) {
                return self.drop_aura(i);
            }
            let slot = self.auras[i].slot;
            self.auras[i].ends = ends;
            out.push(Out::Pkt(SMSG_AURA_UPDATE, aura_body(g, slot, s.id, self.me, mask, !harmful(s), self.stats.level as u8, (ends > 0).then_some((dur, dur)))));
            return out;
        }
        if form != 0 {
            // one form at a time
            if let Some(i) = self.auras.iter().position(|a| a.unit == g && a.form != 0) {
                out.extend(self.drop_aura(i));
            }
            self.form = form;
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::BYTES_2, (form as u32) << 24)])));
        }
        if mount != 0 && g == self.me {
            // one mount at a time
            if let Some(i) = self.auras.iter().position(|a| a.unit == g && a.mount != 0) {
                out.extend(self.drop_aura(i));
            }
            let pct = |aura: u32| effs.iter().find(|x| x.1.aura == aura).map(|x| x.2.max(0) as f32).unwrap_or(0.0);
            let (run, flight) = (crate::dojo::BASE_RUN * (1.0 + pct(32) / 100.0), crate::dojo::BASE_FLIGHT * (1.0 + pct(207).max(pct(32)) / 100.0));
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(MOUNT_DISPLAY, if mount == 1 { 0 } else { mount }), (field::FLAGS, FLAGS_MOUNTED)])));
            out.extend(self.speed_packets(run, flight));
        }
        let slot = self.free_slot(g);
        let (mut amount, mut heal, mut aura, mut every) = (0u32, false, effs[0].1.aura, 3000u64);
        if let Some((_, e, value)) = effs.iter().find(|x| is_dot(&x.1) || is_hot(&x.1)) {
            every = if e.amplitude > 0 { e.amplitude as u64 } else { 3000 };
            let ticks = if ends > 0 { (dur as u64 / every).max(1) as u32 } else { 1 };
            let bonus = self.stats.spell_power as f32 * (dur.min(15_000) as f32 / 15_000.0).max(coefficient(cast_ms) * 0.5);
            heal = is_hot(e);
            amount = (*value).max(1) as u32 + if heal { bonus as u32 * 2 / ticks } else { bonus as u32 / ticks };
            aura = e.aura;
        }
        self.auras.push(Aura { unit: g, slot, spell: s.id, ends, every, next: now + every, amount, heal, aura, school: s.school_mask, form, mount });
        out.push(Out::Pkt(SMSG_AURA_UPDATE, aura_body(g, slot, s.id, self.me, mask, !harmful(s), self.stats.level as u8, (ends > 0).then_some((dur, dur)))));
        out
    }

    fn drop_aura(&mut self, i: usize) -> Vec<Out> {
        let a = self.auras.remove(i);
        let mut out = vec![Out::Pkt(SMSG_AURA_UPDATE, aura_body(a.unit, a.slot, 0, self.me, 0, true, 0, None))];
        if a.form != 0 && a.unit == self.me {
            self.form = 0;
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::BYTES_2, 0)])));
        }
        if a.mount != 0 && a.unit == self.me {
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(MOUNT_DISPLAY, 0), (field::FLAGS, FLAGS_PLAYER)])));
            out.extend(self.speed_packets(crate::dojo::BASE_RUN, crate::dojo::BASE_FLIGHT));
        }
        out
    }

    /// CMSG_CANCEL_AURA (a buff right-clicked away).
    pub fn cancel_aura(&mut self, spell: u32) -> Vec<Out> {
        match self.auras.iter().position(|a| a.unit == self.me && a.spell == spell) {
            Some(i) => self.drop_aura(i),
            None => Vec::new(),
        }
    }

    /// The cast in progress stops (moving, or the game's CMSG_CANCEL_CAST); a channel too.
    pub fn interrupt(&mut self, why: u8) -> Vec<Out> {
        let mut out = Vec::new();
        if let Some(c) = self.casting.take() {
            out.push(Out::Pkt(SMSG_SPELL_FAILURE, interrupted_body(self.me, c.req.count, c.spell.id, why)));
            out.push(Out::Pkt(SMSG_SPELL_FAILED_OTHER, interrupted_body(self.me, c.req.count, c.spell.id, why)));
            out.push(Out::Pkt(SMSG_CAST_FAILED, failed_body(c.req.count, c.spell.id, why)));
            self.gcd_until = 0;
        }
        if self.channel.is_some() {
            out.extend(self.end_channel());
        }
        out
    }

    /// The player moved: a cast with a cast time stops (channels too).
    pub fn moved(&mut self) -> Vec<Out> {
        if self.casting.is_some() || self.channel.is_some() { self.interrupt(fail::INTERRUPTED) } else { Vec::new() }
    }

    fn end_channel(&mut self) -> Vec<Out> {
        self.channel = None;
        vec![
            Out::Pkt(MSG_CHANNEL_UPDATE, channel_update_body(self.me, 0)),
            Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::CHANNEL_OBJECT, 0), (field::CHANNEL_OBJECT + 1, 0), (field::CHANNEL_SPELL, 0)])),
        ]
    }

    /// `.den dojo cooldowns`: every cooldown cleared.
    pub fn clear_cooldowns(&mut self) -> Vec<Out> {
        let mut ids: Vec<u32> = self.cooldowns.drain().map(|(id, _)| id).collect();
        ids.sort_unstable();
        self.gcd_until = 0;
        ids.into_iter().map(|id| Out::Pkt(SMSG_CLEAR_COOLDOWN, clear_cooldown_body(id, self.me))).collect()
    }

    /// `.den dojo refill`: full health and power.
    pub fn refill(&mut self) -> Vec<Out> {
        self.health = self.stats.health;
        self.power = self.stats.max_power as f32;
        vec![self.me_update()]
    }

    /// `.den dojo reset`: every dummy full again (the boss up again), the meter and parse cleared.
    pub fn reset_units(&mut self) -> Vec<Out> {
        let mut out = Vec::new();
        let mut gs: Vec<u64> = self.hp.keys().copied().collect();
        gs.sort_unstable();
        for g in gs {
            let h = self.hp.get_mut(&g).unwrap();
            h.cur = if h.kind == Kind::Healer { h.max * 3 / 10 } else { h.max };
            h.last_hit = 0;
            h.swing_next = 0;
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(g, &[(field::HEALTH, h.cur), (field::DYNAMIC_FLAGS, 0)])));
        }
        let ds: Vec<usize> = (0..self.auras.len()).rev().filter(|&i| self.auras[i].unit != self.me).collect();
        for i in ds {
            out.extend(self.drop_aura(i));
        }
        self.meter = Meter::default();
        self.parse = None;
        self.boss = None;
        out
    }

    /// `.den dojo parse <minutes>`: from the next hit.
    pub fn start_parse(&mut self, minutes: u32) {
        self.parse = Some(Parse { minutes, start: 0, total: 0 });
        self.meter = Meter::default();
    }

    /// Every 50 ms: the cast that is due, swings, channel and aura ticks, the dummies' own (the healing dummy
    /// bleeds, the tank hits back, the boss's circles), power and health back, the meter's end.
    pub fn tick(&mut self, now: u64, me: At, units: &[Unit], book: &Book) -> Vec<Out> {
        let mut out = Vec::new();
        if let Some(c) = self.casting.as_ref().filter(|c| now >= c.ends).cloned() {
            self.casting = None;
            out.extend(self.go(now, me, units, book, c.req, c.spell, c.target));
        }
        // the channel's ticks: its damage over time, or the spell it triggers (with that spell's own look)
        if let Some(ch) = self.channel.clone() {
            if now >= ch.next {
                if let Some(c) = self.channel.as_mut() {
                    c.next += c.every;
                }
                let level = self.stats.level;
                for (k, e) in ch.spell.effects.iter().enumerate() {
                    let _ = k;
                    if e.kind == 0 {
                        continue;
                    }
                    let (spell, eff) = if e.aura == 23 && e.trigger != 0 {
                        match book.spell(e.trigger).and_then(|t| t.effects.iter().find(|x| is_damage(x) || is_heal(x)).map(|x| (t.clone(), *x))) {
                            Some(p) => p,
                            None => continue,
                        }
                    } else {
                        (ch.spell.clone(), *e)
                    };
                    let targets = self.touched(&spell, &eff, me, units, book, &ch.req, ch.target);
                    let roll = self.roll();
                    let ticks = (book.duration_ms(&ch.spell, level).max(1000) as u64 / ch.every).max(1) as f32;
                    let value = effect_value(&spell, &eff, level, roll) + (self.stats.spell_power as f32 * coefficient(book.duration_ms(&ch.spell, level)) / ticks) as i32;
                    if spell.id != ch.spell.id {
                        let r = CastReq { count: 0, spell: spell.id, mask: TF_UNIT, unit: ch.target, src: None, dest: ch.req.dest };
                        let r = if r.dest.is_some() { CastReq { mask: TF_DEST, unit: None, ..r } } else { r };
                        out.push(Out::Pkt(SMSG_SPELL_GO, go_body(self.me, &r, now as u32, &targets, None)));
                    }
                    for g in targets {
                        if is_heal(&eff) || is_hot(&eff) {
                            out.extend(self.heal(now, g, spell.id, value.max(1) as u32, false, None));
                        } else if spell.id != ch.spell.id {
                            let crit = self.crit();
                            out.extend(self.hurt(now, units, g, spell.id, value.max(1) as u32, spell.school_mask, crit, false));
                        } else {
                            out.push(Out::Pkt(SMSG_PERIODICAURALOG, periodic_body(g, self.me, spell.id, e.aura.max(3), value.max(1) as u32, spell.school_mask, false)));
                            out.extend(self.hurt(now, units, g, spell.id, value.max(1) as u32, spell.school_mask, false, true));
                        }
                    }
                }
            }
            if now >= ch.ends {
                out.extend(self.end_channel());
            }
        }
        // swings
        if let Some((g, due, ranged)) = self.swing {
            if now >= due {
                match units.iter().find(|u| u.guid == g).copied() {
                    None => self.swing = None,
                    Some(u) => {
                        let reach = if ranged { RANGED } else { MELEE };
                        let every = if ranged { 2000 } else { self.stats.attack_ms as u64 };
                        if dist(me, u.at) > reach + 1.0 {
                            if !ranged {
                                out.push(Out::Pkt(SMSG_ATTACKSWING_NOTINRANGE, Vec::new()));
                            }
                            self.swing = Some((g, now + 500, ranged));
                        } else {
                            self.swing = Some((g, now + every, ranged));
                            let crit = self.crit();
                            let r = self.roll();
                            let raw = weapon_hit(&self.stats, r) * if crit { 2 } else { 1 };
                            // against a higher level: misses, dodges, glancing blows (auto attacks, a quarter less)
                            let missed = self.missed(units, g, false);
                            let glancing = missed.is_none() && !crit && u.level > self.stats.level + 2 && self.chance(24.0);
                            let dmg = if missed.is_some() { 0 } else { self.dealt(units, g, raw as f32 * if glancing { 0.75 } else { 1.0 }, 1) };
                            if ranged {
                                let req = CastReq { count: 0, spell: classes::AUTO_SHOT, mask: TF_UNIT, unit: Some(g), src: None, dest: None };
                                match missed {
                                    Some(why) => {
                                        out.push(Out::Pkt(SMSG_SPELL_GO, go_body_missed(self.me, &req, now as u32, &[], &[(g, why)], None)));
                                        out.push(Out::Pkt(SMSG_SPELLLOGMISS, spell_miss_body(classes::AUTO_SHOT, self.me, g, why)));
                                    }
                                    None => {
                                        out.push(Out::Pkt(SMSG_SPELL_GO, go_body(self.me, &req, now as u32, &[g], None)));
                                        out.extend(self.hurt(now, units, g, classes::AUTO_SHOT, dmg, 1, crit, false));
                                        out.extend(self.procs(now, me, units, book, g));
                                    }
                                }
                            } else {
                                let (hit, state) = swing_outcome(crit, glancing, missed == Some(MISS), missed == Some(DODGE));
                                out.push(Out::Pkt(SMSG_ATTACKERSTATEUPDATE, swing_outcome_body(self.me, g, dmg, hit, state)));
                                if dmg > 0 {
                                    out.extend(self.took(now, &u, dmg as i64));
                                    out.extend(self.procs(now, me, units, book, g));
                                }
                                if self.stats.power == classes::RAGE {
                                    self.power = (self.power + 100.0).min(self.stats.max_power as f32);
                                }
                            }
                        }
                    }
                }
            }
        }
        // auras: ticks and their end
        let mut i = 0;
        while i < self.auras.len() {
            let a = self.auras[i].clone();
            if a.amount > 0 && a.every > 0 && now >= a.next {
                self.auras[i].next += a.every;
                if a.heal {
                    out.extend(self.heal(now, a.unit, a.spell, a.amount, false, Some(a.aura)));
                } else if units.iter().any(|u| u.guid == a.unit) {
                    out.push(Out::Pkt(SMSG_PERIODICAURALOG, periodic_body(a.unit, self.me, a.spell, a.aura, a.amount, a.school, false)));
                    out.extend(self.hurt(now, units, a.unit, a.spell, a.amount, a.school, false, true));
                }
            }
            if a.ends > 0 && now >= a.ends {
                if let Some(j) = self.auras.iter().position(|x| x.unit == a.unit && x.slot == a.slot) {
                    out.extend(self.drop_aura(j));
                }
                continue;
            }
            i += 1;
        }
        out.extend(self.dummies_tick(now, me, units));
        out.extend(self.pet_tick(now, me, units));
        // power and health back, once a second
        if now >= self.next_regen {
            self.next_regen = now + 1000;
            let max = self.stats.max_power as f32;
            let gain = match self.stats.power {
                classes::MANA => max * 0.03,
                classes::ENERGY => 10.0,
                _ => 30.0,
            };
            let before = (self.power as u32, self.health);
            self.power = (self.power + gain).min(max);
            if now >= self.last_hurt + 5000 {
                self.health = (self.health + (self.stats.health / 20).max(1)).min(self.stats.health);
            }
            if (self.power as u32, self.health) != before {
                out.push(self.me_update());
            }
        }
        // the fight is over: its numbers (damage, healing, or both)
        let last = self.meter.last.max(self.heals.last);
        if (self.meter.total > 0 || self.heals.total > 0) && now >= last + IDLE_MS && self.parse.as_ref().map_or(true, |p| p.start == 0) && self.demo.is_none() {
            let line = self.readout(true);
            let (m, h) = (std::mem::take(&mut self.meter), std::mem::take(&mut self.heals));
            let mut parts = Vec::new();
            if m.total > 0 {
                parts.push(format!("{} damage in {:.1} s: {} per second", m.total, m.secs(), m.per_sec()));
            }
            if h.total > 0 {
                parts.push(format!("{} healing in {:.1} s: {} per second", h.total, h.secs(), h.per_sec()));
            }
            out.push(Out::Dps(line.clone()));
            out.push(Out::Say(format!("{}.", parts.join("; "))));
            out.push(Out::Event("dps", line));
        }
        out.extend(self.demo_tick(now, me, units, book));
        if let Some(p) = self.parse.clone() {
            if p.start > 0 && now >= p.start + p.minutes as u64 * 60_000 {
                self.parse = None;
                let secs = p.minutes as f32 * 60.0;
                let dps = (p.total as f32 / secs).round() as u32;
                out.push(Out::Parse { minutes: p.minutes, dps, total: p.total, secs });
                out.extend(self.stop());
                self.meter = Meter::default();
                self.heals = Meter::default();
            }
        }
        out
    }

    /// The dummies' own doings: the healing dummy loses health, a dummy that hits back swings at you, the boss's
    /// circles (a cast at your feet you have 2.5 s to leave, then its hit).
    fn dummies_tick(&mut self, now: u64, me: At, units: &[Unit]) -> Vec<Out> {
        let mut out = Vec::new();
        for u in units {
            let Some(h) = self.hp.get(&u.guid).copied() else { continue };
            if u.kind == Kind::Healer && now >= h.swing_next {
                if let Some(x) = self.hp.get_mut(&u.guid) {
                    x.swing_next = now + 1000;
                    // bleeds a fiftieth a second, never below a twentieth (always something to heal)
                    x.cur = x.cur.saturating_sub(x.max / 50).max(x.max / 20);
                    out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, x.cur)])));
                }
            }
            if u.kind == Kind::Group && now >= h.swing_next {
                // the healing range: each loses a little every second, one (the first: the "tank") more, and now and
                // then one of them takes a big hit
                let big = self.roll2() % 5 == 0;
                let who = self.roll2() % 5;
                let first = units.iter().find(|x| x.kind == Kind::Group).map(|x| x.guid) == Some(u.guid);
                let nth = units.iter().filter(|x| x.kind == Kind::Group).position(|x| x.guid == u.guid).unwrap_or(0) as u32;
                if let Some(x) = self.hp.get_mut(&u.guid) {
                    x.swing_next = now + 1000;
                    let mut loss = x.max / if first { 40 } else { 100 };
                    if big && who == nth {
                        loss += x.max / 6;
                    }
                    x.cur = x.cur.saturating_sub(loss).max(x.max / 20);
                    out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, x.cur)])));
                }
            }
            let hits_back = u.kind == Kind::Tank || (self.fight && u.kind.hostile() && u.kind != Kind::Boss && u.kind != Kind::Raid);
            if hits_back && h.cur > 0 && h.last_hit > 0 && now < h.last_hit + 10_000 && h.swing_next > 0 && now >= h.swing_next && dist(me, u.at) <= MELEE + 2.0 {
                if let Some(x) = self.hp.get_mut(&u.guid) {
                    x.swing_next = now + HIT_BACK_MS;
                }
                let dmg = (self.stats.health / 20).max(1) + self.roll() % (self.stats.health / 40).max(1);
                out.push(Out::Pkt(SMSG_ATTACKERSTATEUPDATE, swing_body(u.guid, self.me, dmg, false)));
                out.extend(self.hurt_me(now, dmg));
            }
        }
        // the boss
        if let Some(mut b) = self.boss.clone() {
            if !units.iter().any(|u| u.guid == b.guid) {
                self.boss = None;
                return out;
            }
            if let Some((at, due)) = b.circle {
                if now >= due {
                    b.circle = None;
                    let r = CastReq { count: b.count, spell: BOSS_SPELL, mask: TF_DEST, unit: None, src: None, dest: Some(at) };
                    let inside = ((me.x - at[0]).powi(2) + (me.y - at[1]).powi(2)).sqrt() <= BOSS_RADIUS;
                    out.push(Out::Pkt(SMSG_SPELL_GO, go_body(b.guid, &r, now as u32, if inside { &[] } else { &[] }, None)));
                    if inside {
                        b.hits += 1;
                        out.push(Out::Middle("The circle caught you.".into()));
                        if self.fight {
                            let dmg = self.stats.health / 5;
                            out.extend(self.hurt_me(now, dmg));
                        }
                    }
                }
            } else if b.casting.is_none() && now >= b.next_circle {
                b.next_circle = now + BOSS_EVERY_MS;
                b.count = b.count.wrapping_add(1);
                let at = [me.x, me.y, me.z];
                b.circle = Some((at, now + BOSS_WARN_MS as u64));
                let r = CastReq { count: b.count, spell: BOSS_SPELL, mask: TF_DEST, unit: None, src: None, dest: Some(at) };
                let mut body = Vec::new();
                packed_guid(&mut body, b.guid);
                packed_guid(&mut body, b.guid);
                body.push(r.count);
                body.extend_from_slice(&BOSS_SPELL.to_le_bytes());
                body.extend_from_slice(&CAST_FLAG_START.to_le_bytes());
                body.extend_from_slice(&BOSS_WARN_MS.to_le_bytes());
                write_targets(&mut body, r.mask, None, None, r.dest);
                out.push(Out::Pkt(SMSG_SPELL_START, body));
                out.push(Out::Middle("Fire at your feet: move out!".into()));
                out.push(Out::Event("dojo", "boss circle".into()));
            }
            // its cast at you: interrupt it (a kick, a silence, a stun) before it lands
            match b.casting {
                Some((due, count)) if now >= due => {
                    b.casting = None;
                    let r = CastReq { count, spell: BOSS_CAST_SPELL, mask: TF_UNIT, unit: Some(self.me), src: None, dest: None };
                    out.push(Out::Pkt(SMSG_SPELL_GO, go_body(b.guid, &r, now as u32, &[self.me], None)));
                    out.push(Out::Middle("Its cast hit you: interrupt it next time.".into()));
                    let dmg = self.stats.health / 4;
                    out.push(Out::Pkt(SMSG_SPELLNONMELEEDAMAGELOG, spell_damage_body(self.me, b.guid, BOSS_CAST_SPELL, if self.god { 0 } else { dmg }, 0, 32, false)));
                    out.extend(self.hurt_me(now, dmg));
                }
                None if b.circle.is_none() && now >= b.next_cast => {
                    b.next_cast = now + BOSS_CAST_EVERY_MS;
                    b.count = b.count.wrapping_add(1);
                    b.casts += 1;
                    b.casting = Some((now + BOSS_CAST_MS as u64, b.count));
                    let mut body = Vec::new();
                    packed_guid(&mut body, b.guid);
                    packed_guid(&mut body, b.guid);
                    body.push(b.count);
                    body.extend_from_slice(&BOSS_CAST_SPELL.to_le_bytes());
                    body.extend_from_slice(&CAST_FLAG_START.to_le_bytes());
                    body.extend_from_slice(&BOSS_CAST_MS.to_le_bytes());
                    write_targets(&mut body, TF_UNIT, Some(self.me), None, None);
                    out.push(Out::Pkt(SMSG_SPELL_START, body));
                    out.push(Out::Middle("The boss is casting: interrupt it!".into()));
                    out.push(Out::Event("dojo", "boss cast".into()));
                }
                _ => {}
            }
            self.boss = Some(b);
        }
        out
    }

    /// The player is hurt (a dummy that hits back, the boss's circle with fight on): never below 1, back to full.
    fn hurt_me(&mut self, now: u64, dmg: u32) -> Vec<Out> {
        self.last_hurt = now;
        let mut out = Vec::new();
        if self.god {
            return out;
        }
        // a tank's armour and avoidance (the gear's tier)
        let dmg = (dmg as f32 * (1.0 - armor_reduction(self.mods.armor, 83).min(0.6)) * (1.0 - self.mods.avoid / 100.0)) as u32;
        if dmg >= self.health {
            self.health = self.stats.health;
            out.push(Out::Middle("That would have been the end of you. Back to full.".into()));
        } else {
            self.health -= dmg;
        }
        out.push(self.me_update());
        out
    }

    /// The pet a spell calls (SPELL_EFFECT_SUMMON_PET 56 or SUMMON 28 naming a pet Den Lite knows; Call Pet: the
    /// hunter's wolf; Raise Dead: the ghoul).
    fn summoned(&self, s: &Spell) -> Option<u32> {
        for e in &s.effects {
            if e.kind == 56 || e.kind == 28 {
                let m = e.misc.max(0) as u32;
                if pets::pet_def(m).is_some() {
                    return Some(m);
                }
                if e.kind == 56 && self.kit.as_ref().is_some_and(|k| k.class == 3) {
                    return Some(pets::HUNTER_PET);
                }
            }
        }
        match s.name.as_str() {
            "Call Pet" => Some(pets::HUNTER_PET),
            "Raise Dead" => Some(pets::GHOUL),
            _ => None,
        }
    }

    /// A pet at your side (the old one sent away first): its create, your UNIT_FIELD_SUMMON, its bar.
    pub fn summon(&mut self, now: u64, me: At, entry: u32) -> Vec<Out> {
        let Some(def) = pets::pet_def(entry).copied() else { return Vec::new() };
        let mut out = self.dismiss();
        self.pets_made += 1;
        let guid = pets::pet_guid(entry, self.pets_made);
        let at = At { x: me.x - me.o.cos() * 2.0 - me.o.sin() * 1.5, y: me.y - me.o.sin() * 2.0 + me.o.cos() * 1.5, z: me.z, o: me.o };
        let health = (self.stats.health / 2).max(100);
        let fields = pets::pet_fields(entry, self.me, self.stats.level, health, pets::pet_display(entry));
        out.push(Out::Pkt(SMSG_UPDATE_OBJECT, pets::create_unit_body(guid, [at.x, at.y, at.z, at.o], now as u32, &fields)));
        out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(pets::UNIT_FIELD_SUMMON, guid as u32), (pets::UNIT_FIELD_SUMMON + 1, (guid >> 32) as u32)])));
        let spells: Vec<u32> = if def.bolt != 0 { vec![def.bolt] } else { Vec::new() };
        out.push(Out::Pkt(pets::SMSG_PET_SPELLS, pets::pet_spells_body(guid, def.family, 1, 1, &spells)));
        out.push(Out::Event("pet", format!("{} {entry}", def.name)));
        self.pet = Some(Pet { guid, def, at, target: 0, next_hit: now + 1000, next_move: now + 500, react: 1, stay: false, move_id: 1 });
        out
    }

    /// The pet sent away: gone from the world and from your bar.
    pub fn dismiss(&mut self) -> Vec<Out> {
        let Some(p) = self.pet.take() else { return Vec::new() };
        vec![
            Out::Pkt(pets::SMSG_DESTROY_OBJECT, pets::destroy_body(p.guid)),
            Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(pets::UNIT_FIELD_SUMMON, 0), (pets::UNIT_FIELD_SUMMON + 1, 0)])),
            Out::Pkt(pets::SMSG_PET_SPELLS, pets::no_pet_body()),
            Out::Event("pet", "away".into()),
        ]
    }

    /// CMSG_PET_ACTION: Attack (your target), Follow, Stay, Dismiss (ACT_COMMAND 7); Aggressive, Defensive, Passive
    /// (ACT_REACTION 6). Its spell buttons: it already uses its bolt.
    pub fn pet_action(&mut self, units: &[Unit], body: &[u8]) -> Vec<Out> {
        let Some((guid, action, kind, target)) = pets::read_pet_action(body) else { return Vec::new() };
        if self.pet.as_ref().map(|p| p.guid) != Some(guid) {
            return Vec::new();
        }
        match (kind, action) {
            (7, 3) => return self.dismiss(),
            (7, a) => {
                let hostile = units.iter().any(|u| u.guid == target && u.kind.hostile());
                if let Some(p) = self.pet.as_mut() {
                    match a {
                        0 => (p.stay, p.target) = (true, 0),
                        1 => (p.stay, p.target) = (false, 0),
                        _ if hostile => p.target = target,
                        _ => {}
                    }
                }
            }
            (6, r) => {
                if let Some(p) = self.pet.as_mut() {
                    p.react = r.min(2) as u8;
                    if p.react == 0 {
                        p.target = 0;
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// Every tick: the pet fights its target (the one you sent it at, else, unless passive, what you fight): it
    /// walks up to it, then swings (or casts its bolt) every two seconds, its damage counted with yours; with
    /// nothing to fight it follows you (unless told to stay).
    fn pet_tick(&mut self, now: u64, me: At, units: &[Unit]) -> Vec<Out> {
        let mut out = Vec::new();
        let Some(mut p) = self.pet.clone() else { return out };
        let alive = |g: u64, c: &Combat| g != 0 && units.iter().any(|u| u.guid == g && u.kind.hostile()) && c.hp.get(&g).map_or(true, |h| h.cur > 0);
        if !alive(p.target, self) {
            p.target = 0;
        }
        if p.target == 0 && p.react > 0 && alive(self.last_target, self) && self.hp.get(&self.last_target).is_some_and(|h| h.last_hit + 10_000 > now) {
            p.target = self.last_target;
        }
        let reach = if p.def.bolt != 0 { PET_BOLT_REACH } else { PET_REACH };
        match units.iter().find(|u| u.guid == p.target).copied() {
            Some(u) => {
                let d = dist(p.at, u.at);
                if d > reach && now >= p.next_move {
                    // up to it, stopping just short
                    let k = ((d - reach * 0.6) / d).clamp(0.0, 1.0);
                    let to = At { x: p.at.x + (u.at.x - p.at.x) * k, y: p.at.y + (u.at.y - p.at.y) * k, z: u.at.z, o: p.at.o };
                    let ms = (dist(p.at, to) / 7.0 * 1000.0) as u32;
                    p.move_id = p.move_id.wrapping_add(1);
                    out.push(Out::Pkt(pets::SMSG_MONSTER_MOVE, pets::move_body(p.guid, [p.at.x, p.at.y, p.at.z], [to.x, to.y, to.z], p.move_id, ms)));
                    p.at = to;
                    p.next_move = now + ms as u64 + 100;
                    p.next_hit = p.next_hit.max(p.next_move);
                } else if d <= reach + 1.0 && now >= p.next_hit {
                    p.next_hit = now + PET_HIT_MS;
                    let crit = self.roll2() % 100 < 5;
                    let base = (self.stats.level.max(1) * if p.def.bolt != 0 { 12 } else { 9 }) as f32 * (1.0 + classes::tier_ilvl(self.kit.as_ref().map(|k| k.tier).unwrap_or(0)) / 200.0);
                    let school = if p.def.bolt != 0 { 4 } else { 1 };
                    let dmg = self.dealt(units, u.guid, base * if crit { 1.5 } else { 1.0 }, school);
                    if p.def.bolt != 0 {
                        let r = CastReq { count: 0, spell: p.def.bolt, mask: TF_UNIT, unit: Some(u.guid), src: None, dest: None };
                        out.push(Out::Pkt(SMSG_SPELL_GO, go_body(p.guid, &r, now as u32, &[u.guid], None)));
                        out.push(Out::Pkt(SMSG_SPELLNONMELEEDAMAGELOG, spell_damage_body(u.guid, p.guid, p.def.bolt, dmg, 0, school, crit)));
                    } else {
                        out.push(Out::Pkt(SMSG_ATTACKERSTATEUPDATE, swing_body(p.guid, u.guid, dmg, crit)));
                    }
                    self.unit_health(&u);
                    out.extend(self.took(now, &u, dmg as i64));
                }
            }
            None if !p.stay && now >= p.next_move && dist(p.at, me) > PET_FOLLOW => {
                let to = At { x: me.x - me.o.cos() * 2.0 - me.o.sin() * 1.5, y: me.y - me.o.sin() * 2.0 + me.o.cos() * 1.5, z: me.z, o: me.o };
                let ms = (dist(p.at, to) / 7.0 * 1000.0).min(4000.0) as u32;
                p.move_id = p.move_id.wrapping_add(1);
                out.push(Out::Pkt(pets::SMSG_MONSTER_MOVE, pets::move_body(p.guid, [p.at.x, p.at.y, p.at.z], [to.x, to.y, to.z], p.move_id, ms)));
                p.at = to;
                p.next_move = now + ms as u64 + 300;
            }
            None => {}
        }
        if self.pet.as_ref().map(|x| x.guid) == Some(p.guid) {
            self.pet = Some(p);
        }
        out
    }

    /// `.den dojo demo [seconds]`: Den Lite casts your main bar at `target` for you (a short timed parse, and a way
    /// to see a rotation); the meters start again.
    pub fn start_demo(&mut self, now: u64, target: u64, secs: u32) {
        self.meter = Meter::default();
        self.heals = Meter::default();
        self.demo = Some(Demo { target, start: now, until: now + secs.clamp(5, 300) as u64 * 1000, total: 0, count: 100 });
    }

    /// The practice run's next cast: of the main bar's spells that suit the target (harmful at a dummy, heals at a
    /// friend), those with a cooldown when ready and damage or healing over time not on it yet first, then the rest
    /// in the bar's order; the auto attack on too. At its end: its numbers.
    fn demo_tick(&mut self, now: u64, me: At, units: &[Unit], book: &Book) -> Vec<Out> {
        let Some(d) = self.demo.clone() else { return Vec::new() };
        if now >= d.until {
            self.demo = None;
            // the run is over: a cast still going stops, the swings too
            let mut out = self.interrupt(fail::INTERRUPTED);
            out.extend(self.stop());
            let secs = (d.until - d.start) as f32 / 1000.0;
            let dps = (d.total as f32 / secs).round() as u32;
            out.push(Out::Dps(self.readout(true)));
            out.push(Out::Demo { secs, total: d.total, dps });
            self.meter = Meter::default();
            self.heals = Meter::default();
            return out;
        }
        if self.casting.is_some() || self.channel.is_some() || now + SLACK_MS < self.gcd_until {
            return Vec::new();
        }
        let Some(u) = units.iter().find(|u| u.guid == d.target).copied() else {
            self.demo = None;
            return vec![Out::Say("The practice run stopped: its dummy is gone.".into())];
        };
        let mut out = Vec::new();
        let hostile = u.kind.hostile();
        if hostile && self.swing.is_none() {
            let ranged = self.knows(classes::AUTO_SHOT) && book.spell(classes::AUTO_SHOT).is_some();
            if ranged || dist(me, u.at) <= MELEE + 1.0 {
                out.extend(self.attack(now, units, u.guid));
                if ranged {
                    if let Some(sw) = self.swing.as_mut() {
                        sw.2 = true;
                    }
                    out.retain(|o| !matches!(o, Out::Pkt(SMSG_ATTACKSTART, _)));
                }
            }
        }
        let bar: Vec<u32> = self.kit.as_ref().map(|k| k.buttons.iter().take(12).filter(|b| *b >> 24 == 0).map(|b| b & 0x00FF_FFFF).collect()).unwrap_or_default();
        let on_target = self.auras_on(u.guid);
        let mut best: Option<(u8, usize, u32)> = None;
        for (i, &id) in bar.iter().enumerate() {
            if id == 0 || id == classes::ATTACK || id == classes::AUTO_SHOT || !self.knows(id) {
                continue;
            }
            let Some(s) = book.spell(id) else { continue };
            let heals = s.effects.iter().any(|e| is_heal(e) || is_hot(e));
            let fits = if hostile { harmful(s) } else { heals && !harmful(s) };
            let odd = s.passive() || s.effects.iter().any(|e| matches!(e.kind, 28 | 56) || (e.kind == 6 && matches!(e.aura, 36 | 78)));
            if !fits || odd || self.cooldowns.get(&id).is_some_and(|t| now + SLACK_MS < *t) || self.cost(s) > self.power + 0.5 {
                continue;
            }
            let over_time = s.effects.iter().any(|e| is_dot(e) || is_hot(e));
            if over_time && on_target.contains(&id) {
                continue;
            }
            let score = if s.recovery > 0 || s.category_recovery > 0 { 0 } else if over_time { 1 } else { 2 };
            if best.is_none_or(|b| (score, i) < (b.0, b.1)) {
                best = Some((score, i, id));
            }
        }
        if let Some((_, _, id)) = best {
            let count = d.count.wrapping_add(1).max(100);
            if let Some(x) = self.demo.as_mut() {
                x.count = count;
            }
            let r = CastReq { count, spell: id, mask: TF_UNIT, unit: Some(u.guid), src: None, dest: None };
            let got = self.cast(now, me, units, book, r);
            if got.iter().any(|o| matches!(o, Out::Pkt(SMSG_CAST_FAILED, _))) {
                // not now (out of range, ...): try again in a moment
                self.gcd_until = self.gcd_until.max(now + 500);
            }
            out.extend(got);
        }
        out
    }

    /// Is a cast going (the tests, `.den where`).
    pub fn casting(&self) -> bool {
        self.casting.is_some()
    }
    pub fn channeling(&self) -> bool {
        self.channel.is_some()
    }
    pub fn auras_on(&self, unit: u64) -> Vec<u32> {
        self.auras.iter().filter(|a| a.unit == unit).map(|a| a.spell).collect()
    }
}

/// A dummy's name (the meter's), as dojo.rs names them.
pub fn dummy_name(k: Kind) -> &'static str {
    match k {
        Kind::Target => "Training Dummy",
        Kind::Pack => "Pack Dummy",
        Kind::Mover => "Moving Dummy",
        Kind::Healer => "Wounded Dummy",
        Kind::Tank => "Sparring Dummy",
        Kind::Boss => "Practice Boss",
        Kind::Friend => "Companion",
        Kind::Light => "Novice's Training Dummy",
        Kind::Heavy => "Raider's Training Dummy",
        Kind::Group => "Wounded Friend",
        Kind::Marker => "Range Marker",
        Kind::Raid => "Raid Boss",
    }
}

pub fn dist(a: At, b: At) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::{Pick, CLASSES};
    use crate::spells::fake;

    fn book() -> Book {
        fake::book()
    }

    fn mage(b: &Book) -> Combat {
        let mut c = Combat::new(1);
        c.seed(7);
        c.set_kit(classes::kit(b, &Pick { class: 8, level: 80, ..Default::default() }));
        c
    }

    const DUMMY: u64 = 0xF130_0079_A800_0101;
    fn here() -> At {
        At { x: 0.0, y: 0.0, z: 0.0, o: 0.0 }
    }
    fn units(kind: Kind) -> Vec<Unit> {
        vec![Unit { guid: DUMMY, at: At { x: 5.0, y: 0.0, z: 0.0, o: 0.0 }, kind, level: 80 }]
    }
    fn on(r: u32, unit: u64) -> CastReq {
        CastReq { count: 3, spell: r, mask: TF_UNIT, unit: Some(unit), src: None, dest: None }
    }
    fn ops(out: &[Out]) -> Vec<u16> {
        out.iter().filter_map(|o| if let Out::Pkt(op, _) = o { Some(*op) } else { None }).collect()
    }
    fn body(out: &[Out], op: u16) -> Vec<u8> {
        out.iter().find_map(|o| match o { Out::Pkt(x, b) if *x == op => Some(b.clone()), _ => None }).unwrap_or_else(|| panic!("no {op:#x} in {out:?}"))
    }

    #[test]
    fn a_cast_request_reads_its_targets() {
        let mut b = vec![3];
        b.extend_from_slice(&133u32.to_le_bytes());
        b.push(0);
        b.extend_from_slice(&TF_UNIT.to_le_bytes());
        packed_guid(&mut b, DUMMY);
        let r = read_cast(&b).unwrap();
        assert_eq!(r, on(133, DUMMY));
        // a place: the transport's packed guid, then x y z
        let mut p = vec![1];
        p.extend_from_slice(&190u32.to_le_bytes());
        p.push(0);
        p.extend_from_slice(&TF_DEST.to_le_bytes());
        p.push(0);
        for v in [1.0f32, 2.0, 3.0] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        let r = read_cast(&p).unwrap();
        assert_eq!((r.dest, r.unit), (Some([1.0, 2.0, 3.0]), None));
        assert!(read_cast(&p[..12]).is_none());
        assert_eq!(read_guid(&DUMMY.to_le_bytes()), Some(DUMMY));
    }

    #[test]
    fn the_bodies_byte_for_byte() {
        // a values update: one block, the type, the packed guid, the mask, the values in field order
        let v = values_body(1, &[(field::HEALTH, 500), (field::POWER1, 7)]);
        assert_eq!(v, vec![1, 0, 0, 0, 0, 0x01, 0x01, 1, 0, 0, 0x00, 0x03, 0xF4, 1, 0, 0, 7, 0, 0, 0]);
        let v = values_body(1, &[(field::BYTES_2, 1 << 24)]);
        assert_eq!(v[7], 4, "four blocks reach field 122");
        assert_eq!(u32::from_le_bytes(v[8 + 12..8 + 16].try_into().unwrap()), 1 << (122 - 96));
        assert_eq!(attack_start_body(1, 2), [1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(attack_stop_body(1, 2), [1, 1, 1, 2, 0, 0, 0, 0]);
        let s = swing_body(1, 2, 300, true);
        assert_eq!(&s[..4], &0x82u32.to_le_bytes());
        assert_eq!(&s[4..8], &[1, 1, 1, 2]);
        assert_eq!(u32::from_le_bytes(s[8..12].try_into().unwrap()), 300);
        assert_eq!(s[16], 1, "one damage part");
        assert_eq!(s.len(), 16 + 1 + 12 + 1 + 8);
        let d = spell_damage_body(2, 1, 133, 900, 0, 4, false);
        assert_eq!(&d[..4], &[1, 2, 1, 1]);
        assert_eq!(u32::from_le_bytes(d[4..8].try_into().unwrap()), 133);
        assert_eq!(u32::from_le_bytes(d[8..12].try_into().unwrap()), 900);
        assert_eq!(d[16], 4, "fire");
        assert_eq!(d.len(), 16 + 1 + 4 + 4 + 2 + 4 + 4 + 1);
        let h = heal_body(1, 1, 2061, 1000, 400, true);
        assert_eq!(h.len(), 4 + 4 + 4 + 4 + 4 + 2);
        assert_eq!(h[20], 1);
        let p = periodic_body(2, 1, 172, 3, 50, 32, false);
        assert_eq!(u32::from_le_bytes(p[8..12].try_into().unwrap()), 1, "one aura");
        assert_eq!(u32::from_le_bytes(p[12..16].try_into().unwrap()), 3);
        assert_eq!(p.len(), 4 + 4 + 4 + 4 + 4 * 5 + 1);
        assert_eq!(periodic_body(1, 1, 139, 8, 50, 0, true).len(), 4 + 4 + 4 + 4 + 4 * 3 + 1);
        assert_eq!(failed_body(3, 133, fail::NOT_KNOWN), vec![3, 133, 0, 0, 0, 63, 0]);
        assert_eq!(interrupted_body(1, 3, 133, fail::INTERRUPTED), vec![1, 1, 3, 133, 0, 0, 0, 40]);
        assert_eq!(cooldown_body(1, 2136, 8000).len(), 17);
        assert_eq!(clear_cooldown_body(2136, 1), vec![0x58, 0x08, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(channel_start_body(1, 5143, 3000), vec![1, 1, 0x17, 0x14, 0, 0, 0xB8, 0x0B, 0, 0]);
        // an aura on yourself: no caster guid (0x08), helpful (0x10), with a duration (0x20)
        let a = aura_body(1, 0, 1459, 1, 1, true, 80, Some((1_800_000, 1_800_000)));
        assert_eq!(&a[..7], &[1, 1, 0, 0xB3, 0x05, 0, 0]);
        assert_eq!(a[7], 0x01 | 0x08 | 0x10 | 0x20);
        assert_eq!((a[8], a[9]), (80, 1));
        assert_eq!(a.len(), 10 + 8);
        // a debuff from you on a dummy: your packed guid after the stack
        let d = aura_body(2, 1, 172, 1, 1, false, 80, Some((18000, 18000)));
        assert_eq!(d[7] & 0x88, 0x80);
        assert_eq!(&d[10..12], &[1, 1]);
        assert_eq!(aura_body(1, 4, 0, 1, 0, true, 0, None), vec![1, 1, 4, 0, 0, 0, 0], "a removal");
        assert_eq!(combo_body(2, 3), vec![1, 2, 3]);
    }

    #[test]
    fn start_and_go_carry_the_count_targets_and_power() {
        let r = on(133, 2);
        let s = start_body(1, &r, 2500, 900);
        assert_eq!(&s[..3], &[1, 1, 1]);
        assert_eq!(s[4], 3, "the cast count echoed");
        assert_eq!(u32::from_le_bytes(s[5..9].try_into().unwrap()), 133);
        assert_eq!(u32::from_le_bytes(s[9..13].try_into().unwrap()), 0x802);
        assert_eq!(u32::from_le_bytes(s[13..17].try_into().unwrap()), 2500);
        assert_eq!(u32::from_le_bytes(s[17..21].try_into().unwrap()), TF_UNIT);
        assert_eq!(&s[21..23], &[1, 2]);
        assert_eq!(u32::from_le_bytes(s[23..27].try_into().unwrap()), 900);
        assert_eq!(s.len(), 27);
        let g = go_body(1, &r, 77, &[2], Some(800));
        assert_eq!(u32::from_le_bytes(g[9..13].try_into().unwrap()), 0x900);
        assert_eq!(g[17], 1, "one unit hit");
        assert_eq!(u64::from_le_bytes(g[18..26].try_into().unwrap()), 2);
        assert_eq!(g[26], 0, "none missed");
        assert_eq!(g.len(), 27 + 4 + 2 + 4);
        // at a place: the place, then the byte after it
        let p = CastReq { count: 1, spell: 190, mask: TF_DEST, unit: None, src: None, dest: Some([1.0, 2.0, 3.0]) };
        let g = go_body(1, &p, 0, &[], None);
        assert_eq!(*g.last().unwrap(), 0);
        assert_eq!(g.len(), 2 + 2 + 1 + 4 + 4 + 4 + 2 + 4 + 13 + 1);
    }

    #[test]
    fn values_by_level() {
        let b = book();
        let fb = b.spell(143).unwrap();
        let e = fb.effects[0];
        assert_eq!(effect_value(fb, &e, 80, 0), 31, "base 30 + 1 of 11 die sides");
        assert_eq!(effect_value(fb, &e, 80, 10), 41);
        let mut lvl = fb.clone();
        lvl.effects[0].per_level = 2.0;
        lvl.max_level = 20;
        assert_eq!(effect_value(&lvl, &lvl.effects[0], 80, 0), 31 + 28, "levels over the spell's, held at its max level");
        assert_eq!(coefficient(0), 1500.0 / 3500.0);
        assert_eq!(coefficient(2500), 2500.0 / 3500.0);
        assert_eq!(coefficient(7000), 1.0);
        let st = classes::stats(&CLASSES[0], 80);
        let lo = weapon_hit(&st, 0);
        let hi = weapon_hit(&st, 999);
        assert!(lo >= 520 + 445 && lo < hi && hi <= 780 + 446 + 1, "{lo} {hi}");
        assert!(harmful(fb) && !harmful(b.spell(1459).unwrap()) && harmful(b.spell(5143).unwrap()));
    }

    #[test]
    fn a_fireball_casts_then_lands_on_the_dummy() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        let out = c.cast(1000, here(), &us, &b, on(143, DUMMY));
        assert_eq!(ops(&out), vec![SMSG_SPELL_START]);
        assert_eq!(u32::from_le_bytes(body(&out, SMSG_SPELL_START)[13..17].try_into().unwrap()), 2500, "the cast time from the table");
        assert!(c.casting());
        // too soon for another
        assert_eq!(ops(&c.cast(1100, here(), &us, &b, on(2136, DUMMY))), vec![SMSG_CAST_FAILED]);
        assert!(c.tick(3000, here(), &us, &b).iter().all(|o| !matches!(o, Out::Pkt(SMSG_SPELL_GO, _))));
        let out = c.tick(3500, here(), &us, &b);
        let o = ops(&out);
        assert!(o.contains(&SMSG_SPELL_GO) && o.contains(&SMSG_SPELLNONMELEEDAMAGELOG), "{o:x?}");
        let d = body(&out, SMSG_SPELLNONMELEEDAMAGELOG);
        let mut at = 0;
        assert_eq!((read_packed(&d, &mut at), read_packed(&d, &mut at)), (Some(DUMMY), Some(1)));
        let dmg = u32::from_le_bytes(d[at + 4..at + 8].try_into().unwrap());
        // 31..41 and 1200 spell power * 2.5/3.5, a crit half again
        assert!((31 + 857..=(41 + 858) * 3 / 2).contains(&dmg), "{dmg}");
        let hp = c.hp[&DUMMY];
        assert_eq!(hp.cur, hp.max - dmg);
        let g = body(&out, SMSG_SPELL_GO);
        assert_eq!(u64::from_le_bytes(g[18..26].try_into().unwrap()), DUMMY, "the dummy is hit");
        assert!(c.power < c.stats.max_power as f32, "mana spent");
        assert!(out.iter().any(|o| matches!(o, Out::Dps(l) if l.contains("dummy=Training Dummy"))));
    }

    #[test]
    fn refusals_unknown_friendly_far_and_cooldowns() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        assert_eq!(body(&c.cast(0, here(), &us, &b, on(78, DUMMY)), SMSG_CAST_FAILED)[5], fail::NOT_KNOWN);
        assert_eq!(body(&c.cast(0, here(), &units(Kind::Healer), &b, on(2136, DUMMY)), SMSG_CAST_FAILED)[5], fail::TARGET_FRIENDLY);
        let far = vec![Unit { at: At { x: 80.0, ..here() }, ..us[0] }];
        assert_eq!(body(&c.cast(0, here(), &far, &b, on(2136, DUMMY)), SMSG_CAST_FAILED)[5], fail::OUT_OF_RANGE);
        assert_eq!(body(&c.cast(0, here(), &us, &b, on(2136, 0)), SMSG_CAST_FAILED)[5], fail::BAD_TARGETS);
        // Fire Blast: instant, then its cooldown
        let out = c.cast(0, here(), &us, &b, on(2136, DUMMY));
        assert_eq!(&ops(&out)[..2], &[SMSG_SPELL_START, SMSG_SPELL_GO]);
        assert_eq!(u32::from_le_bytes(body(&out, SMSG_SPELL_COOLDOWN)[13..17].try_into().unwrap()), 8000);
        assert_eq!(body(&c.cast(2000, here(), &us, &b, on(2136, DUMMY)), SMSG_CAST_FAILED)[5], fail::NOT_READY);
        let cleared = c.clear_cooldowns();
        assert_eq!(ops(&cleared), vec![SMSG_CLEAR_COOLDOWN]);
        assert_eq!(ops(&c.cast(2000, here(), &us, &b, on(2136, DUMMY)))[0], SMSG_SPELL_START);
        // no mana
        c.power = 0.0;
        assert_eq!(body(&c.cast(9000, here(), &us, &b, on(143, DUMMY)), SMSG_CAST_FAILED)[5], fail::NO_POWER);
        assert!(c.refill().len() == 1 && c.power == c.stats.max_power as f32);
    }

    #[test]
    fn moving_stops_a_cast_and_the_gcd_comes_back() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        c.cast(0, here(), &us, &b, on(143, DUMMY));
        let out = c.moved();
        assert_eq!(ops(&out), vec![SMSG_SPELL_FAILURE, SMSG_SPELL_FAILED_OTHER, SMSG_CAST_FAILED]);
        assert_eq!(*body(&out, SMSG_SPELL_FAILURE).last().unwrap(), fail::INTERRUPTED);
        assert!(!c.casting() && c.moved().is_empty());
        assert!(c.tick(5000, here(), &us, &b).iter().all(|o| !matches!(o, Out::Pkt(SMSG_SPELL_GO, _))), "nothing lands");
        assert_eq!(ops(&c.cast(100, here(), &us, &b, on(143, DUMMY))), vec![SMSG_SPELL_START]);
    }

    #[test]
    fn a_buff_on_yourself_shows_and_ends() {
        let b = book();
        let mut c = mage(&b);
        let out = c.cast(0, here(), &[], &b, CastReq { count: 1, spell: 1459, mask: 0, ..Default::default() });
        let a = body(&out, SMSG_AURA_UPDATE);
        assert_eq!(&a[..3], &[1, 1, 0], "on you, slot 0");
        assert_eq!(u32::from_le_bytes(a[3..7].try_into().unwrap()), 1459);
        assert_eq!(u64::from_le_bytes(body(&out, SMSG_SPELL_GO)[18..26].try_into().unwrap()), 1, "you are the one hit");
        assert_eq!(c.auras_on(1), vec![1459]);
        assert_eq!(ops(&c.cancel_aura(1459)), vec![SMSG_AURA_UPDATE]);
        assert!(c.auras_on(1).is_empty());
        c.cast(10_000, here(), &[], &b, CastReq { count: 2, spell: 1459, mask: 0, ..Default::default() });
        let out = c.tick(10_000 + 1_800_000, here(), &[], &b);
        assert!(out.iter().any(|o| matches!(o, Out::Pkt(SMSG_AURA_UPDATE, x) if x.len() == 7)), "it runs out");
    }

    #[test]
    fn a_channel_ticks_its_missiles() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        let out = c.cast(0, here(), &us, &b, on(5143, DUMMY));
        assert!(ops(&out).contains(&MSG_CHANNEL_START) && c.channeling());
        let mut gos = 0;
        let mut logs = 0;
        for t in (50..=3100).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                match o {
                    Out::Pkt(SMSG_SPELL_GO, g) if u32::from_le_bytes(g[5..9].try_into().unwrap()) == 7268 => gos += 1,
                    Out::Pkt(SMSG_SPELLNONMELEEDAMAGELOG, _) => logs += 1,
                    _ => {}
                }
            }
        }
        assert_eq!((gos, logs), (3, 3), "a missile a second for 3 s");
        assert!(!c.channeling());
    }

    #[test]
    fn swings_dummies_refill_and_the_numbers_at_the_end() {
        let b = book();
        let mut c = Combat::new(1);
        c.seed(3);
        c.set_kit(classes::kit(&b, &Pick { class: 1, level: 80, ..Default::default() }));
        let us = vec![Unit { guid: DUMMY, at: At { x: 2.0, y: 0.0, z: 0.0, o: 0.0 }, kind: Kind::Target, level: 80 }];
        let out = c.attack(0, &us, DUMMY);
        assert_eq!(ops(&out), vec![SMSG_ATTACKSTART]);
        let mut swings = 0;
        let mut said = None;
        for t in (0..=12_000).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                match o {
                    Out::Pkt(SMSG_ATTACKERSTATEUPDATE, _) => swings += 1,
                    Out::Say(l) => said = Some(l),
                    _ => {}
                }
            }
            if t == 5200 {
                c.stop();
            }
        }
        assert_eq!(swings, 3, "at 0, 2.6 and 5.2 s");
        let line = said.expect("the numbers once the fight is over");
        assert!(line.contains("damage in") && line.contains("per second"), "{line}");
        // a dummy refills at a tenth
        c.hp.get_mut(&DUMMY).unwrap().cur = 1000;
        let out = c.hurt(20_000, &us, DUMMY, 133, 10, 4, false, false);
        let h = body(&out, SMSG_UPDATE_OBJECT);
        assert_eq!(u32::from_le_bytes(h[h.len() - 4..].try_into().unwrap()), 5_000_000);
        assert!(out.iter().any(|o| matches!(o, Out::Event("dummy_down", _))), "a tale counts it as brought down");
        // a healing dummy is friendly and can't be attacked
        assert_eq!(ops(&c.attack(0, &units(Kind::Healer), DUMMY)), vec![SMSG_ATTACKSWING_CANT_ATTACK]);
    }

    #[test]
    fn the_healing_dummy_bleeds_and_takes_heals() {
        let mut c = Combat::new(1);
        let us = units(Kind::Healer);
        let h0 = c.unit_health(&us[0]);
        assert_eq!(h0.cur, h0.max * 3 / 10);
        let b = Book::default();
        c.hp.get_mut(&DUMMY).unwrap().swing_next = 1;
        let out = c.tick(1000, here(), &us, &b);
        assert!(c.hp[&DUMMY].cur < h0.cur, "{out:?}");
        let before = c.hp[&DUMMY].cur;
        let out = c.heal(2000, DUMMY, 2061, 500, false, None);
        assert_eq!(ops(&out)[0], SMSG_SPELLHEALLOG);
        assert_eq!(c.hp[&DUMMY].cur, before + 500);
        assert!(c.heals.healing && c.heals.total == 500 && c.meter.total == 0);
    }

    #[test]
    fn a_tank_dummy_hits_back_but_you_never_fall() {
        let mut c = Combat::new(1);
        let b = Book::default();
        let us = vec![Unit { guid: DUMMY, at: At { x: 2.0, ..here() }, kind: Kind::Tank, level: 80 }];
        c.attack(0, &us, DUMMY);
        let mut hit_back = 0;
        for t in (0..=8000).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                if let Out::Pkt(SMSG_ATTACKERSTATEUPDATE, x) = o {
                    let mut at = 4;
                    if read_packed(&x, &mut at) == Some(DUMMY) {
                        hit_back += 1;
                    }
                }
            }
        }
        assert!(hit_back >= 2, "{hit_back}");
        assert!(c.health >= 1 && c.health <= c.stats.health);
        let out = c.hurt_me(9000, 10_000);
        assert_eq!(c.health, c.stats.health, "back to full, never dead");
        assert!(matches!(out[0], Out::Middle(_)));
    }

    #[test]
    fn the_boss_marks_the_ground_and_falls() {
        let b = book();
        let mut c = mage(&b);
        let boss = 0xF130_0000_0100_0999u64;
        let us = vec![Unit { guid: boss, at: At { x: 10.0, ..here() }, kind: Kind::Boss, level: 80 }];
        c.unit_health(&us[0]);
        let out = c.hurt(0, &us, boss, 133, 100, 4, false, false);
        assert!(out.iter().any(|o| matches!(o, Out::Middle(_))) && c.boss.is_some());
        // the circle: a cast at your feet, 2.5 s, then its hit; you moved: not caught
        let mut started = false;
        for t in (50..=4500).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                if let Out::Pkt(SMSG_SPELL_START, x) = &o {
                    let mut at = 0;
                    started |= read_packed(x, &mut at) == Some(boss);
                }
            }
        }
        assert!(started, "a circle was marked");
        let away = At { x: -20.0, ..here() };
        for t in (4500..=10_000).step_by(50) {
            c.tick(t, away, &us, &b);
        }
        assert!(c.boss.as_ref().unwrap().hits <= 1);
        let max = c.hp[&boss].max;
        let out = c.hurt(11_000, &us, boss, 133, max, 4, false, false);
        assert!(out.iter().any(|o| matches!(o, Out::Boss { .. })), "{out:?}");
        assert!(c.boss.is_none() && c.hp[&boss].cur == 0);
        assert_eq!(body(&c.cast(12_000, here(), &us, &b, on(2136, boss)), SMSG_CAST_FAILED)[5], fail::BAD_TARGETS);
        assert!(c.reset_units().len() >= 1 && c.hp[&boss].cur == max);
    }

    #[test]
    fn a_timed_parse_ends_with_its_numbers() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        c.start_parse(1);
        let out = c.hurt(1000, &us, DUMMY, 133, 6000, 4, false, false);
        assert!(out.iter().any(|o| matches!(o, Out::Say(l) if l.starts_with("Parse started"))));
        let mut done = None;
        for t in (1000..=62_000).step_by(500) {
            for o in c.tick(t, here(), &us, &b) {
                if let Out::Parse { minutes, dps, total, .. } = o {
                    done = Some((minutes, dps, total));
                }
            }
        }
        assert_eq!(done, Some((1, 100, 6000)));
        assert!(c.parse.is_none());
    }

    #[test]
    fn kinds_have_words_factions_and_health() {
        for k in [Kind::Target, Kind::Pack, Kind::Mover, Kind::Healer, Kind::Tank, Kind::Boss] {
            assert_eq!(Kind::from_word(k.word()), Some(k));
            assert!(k.start_health(80) <= k.max_health(80));
            assert!(!crate::companion::branded(dummy_name(k)));
        }
        assert!(!Kind::Healer.hostile() && Kind::Boss.hostile());
        assert_eq!((Kind::Target.faction(), Kind::Tank.faction(), Kind::Healer.faction()), (7, 14, 35));
    }

    #[test]
    fn a_mount_shows_its_model_and_speeds_then_comes_off() {
        let mut b = book();
        // a made-up mount spell: aura 78 on creature 3 (its model 987 in creatures.rs), 100 % run speed
        let mut m = crate::spells::Spell { id: 458, name: "Brown Horse".into(), cast_index: 1, ..Default::default() };
        m.effects[0] = Effect { kind: 6, aura: 78, misc: 3, target_a: 1, ..Default::default() };
        m.effects[1] = Effect { kind: 6, aura: 32, base: 99, die: 1, target_a: 1, ..Default::default() };
        b.spells.insert(458, m);
        let mut c = mage(&b);
        assert!(!c.knows(458));
        c.extra = vec![458];
        assert!(c.knows(458) && c.known().contains(&458) && c.known().contains(&143));
        let out = c.cast(0, here(), &[], &b, CastReq { count: 1, spell: 458, mask: 0, ..Default::default() });
        let mut mounted = None;
        for o in &out {
            if let Out::Pkt(SMSG_UPDATE_OBJECT, v) = o {
                if v.len() > 12 && v[7] == 3 {
                    mounted = Some(v.clone());
                }
            }
        }
        let v = mounted.expect("the mount's fields (three blocks reach field 69)");
        assert_eq!(u32::from_le_bytes(v[v.len() - 8..v.len() - 4].try_into().unwrap()), FLAGS_MOUNTED, "flags (59) come first");
        assert_eq!(u32::from_le_bytes(v[v.len() - 4..].try_into().unwrap()), 987, "the creature's model");
        let run = body(&out, crate::dojo::SMSG_FORCE_RUN_SPEED_CHANGE);
        assert_eq!(f32::from_le_bytes(run[7..11].try_into().unwrap()), 14.0, "twice as fast");
        let auras = ops(&out).iter().filter(|o| **o == SMSG_AURA_UPDATE).count();
        assert_eq!(auras, 1, "one aura for the spell, both effects' bits");
        assert_eq!(body(&out, SMSG_AURA_UPDATE)[7] & 0x3, 0x3);
        assert!(c.mounted());
        let off = c.dismount();
        assert!(!c.mounted());
        let run = body(&off, crate::dojo::SMSG_FORCE_RUN_SPEED_CHANGE);
        assert_eq!(f32::from_le_bytes(run[7..11].try_into().unwrap()), 7.0);
    }
    /// The sample's tables with a kick (Counterspell: interrupts) and a proc's spell.
    fn book_plus() -> Book {
        let mut rows = fake::sample();
        rows.push(fake::Row { id: 2139, name: "Counterspell".into(), level: 24, cast_index: 1, recovery: 24_000, school: 64, effects: vec![(68, 0, 1, 6, 0, 0, 0, 0)], ..Default::default() });
        rows.push(fake::Row { id: 9999, name: "Impact".into(), level: 1, cast_index: 1, school: 4, effects: vec![(2, 100, 1, 6, 0, 0, 0, 0)], ..Default::default() });
        Book::from_bytes(Some(&fake::spells(&rows)), Some(&fake::abilities(&fake::ABILITIES)), Some(&fake::four(&fake::CAST_TIMES)), Some(&fake::four(&fake::DURATIONS)), None)
    }

    fn knows_too(c: &mut Combat, id: u32) {
        let k = c.kit.as_mut().unwrap();
        k.spells.push(id);
        k.spells.sort_unstable();
    }

    #[test]
    fn misses_and_armour_only_against_higher_levels() {
        assert!((armor_reduction(HEAVY_ARMOR, 80) - 0.411).abs() < 0.01, "{}", armor_reduction(HEAVY_ARMOR, 80));
        assert_eq!(armor_reduction(0, 80), 0.0);
        assert_eq!(miss_chances(0, true, 0.0), (0.0, 0.0));
        assert_eq!(miss_chances(-3, false, 0.0), (0.0, 0.0));
        assert_eq!(miss_chances(3, true, 0.0), (17.0, 0.0));
        assert_eq!(miss_chances(3, false, 0.0), (8.0, 6.5));
        assert_eq!(miss_chances(3, true, 100.0), (0.0, 0.0), "hit-capped");
        let b = book();
        let count = |c: &mut Combat, us: &[Unit]| {
            let mut missed = 0;
            for k in 0..60u64 {
                c.cooldowns.clear();
                c.gcd_until = 0;
                c.power = c.stats.max_power as f32;
                let out = c.cast(k * 10_000, here(), us, &b, on(2136, DUMMY));
                missed += out.iter().filter(|o| matches!(o, Out::Pkt(SMSG_SPELLLOGMISS, _))).count();
            }
            missed
        };
        let mut c = mage(&b);
        assert_eq!(count(&mut c, &units(Kind::Target)), 0, "your own level: never");
        let heavy = vec![Unit { level: 83, kind: Kind::Heavy, ..units(Kind::Target)[0] }];
        let m = count(&mut c, &heavy);
        assert!(m > 2 && m < 25, "about 17 per cent of 60: {m}");
        c.mods.hit = 100.0;
        assert_eq!(count(&mut c, &heavy), 0, "hit-capped gear");
        // a miss: the go names it, nothing is taken off
        c.mods.hit = -1000.0;
        c.cooldowns.clear();
        c.gcd_until = 0;
        let before = c.hp[&DUMMY].cur;
        let out = c.cast(900_000, here(), &heavy, &b, on(2136, DUMMY));
        let go = body(&out, SMSG_SPELL_GO);
        let mut at = 0;
        read_packed(&go, &mut at);
        read_packed(&go, &mut at);
        at += 1 + 4 + 4 + 4;
        assert_eq!((go[at], go[at + 1]), (0, 1), "no unit hit, one missed");
        assert_eq!(u64::from_le_bytes(go[at + 2..at + 10].try_into().unwrap()), DUMMY);
        assert_eq!(go[at + 10], MISS);
        assert_eq!(c.hp[&DUMMY].cur, before);
        assert_eq!(spell_miss_body(2136, 1, DUMMY, MISS).len(), 26);
        // a weapon's swings against heavy armour: less
        let mut w = Combat::new(1);
        w.seed(5);
        w.set_kit(classes::kit(&b, &Pick { class: 1, level: 80, ..Default::default() }));
        w.mods.hit = 100.0;
        let near = |k: Kind, l: u32| vec![Unit { guid: DUMMY, at: At { x: 2.0, ..here() }, kind: k, level: l }];
        let total = |w: &mut Combat, us: &[Unit]| -> u64 {
            w.hp.clear();
            w.attack(0, us, DUMMY);
            let mut t = 0;
            for now in (0..=60_000).step_by(100) {
                for o in w.tick(now, here(), us, &b) {
                    if let Out::Pkt(SMSG_ATTACKERSTATEUPDATE, x) = o {
                        let mut at = 4;
                        if read_packed(&x, &mut at) == Some(1) {
                            read_packed(&x, &mut at);
                            t += u32::from_le_bytes(x[at..at + 4].try_into().unwrap()) as u64;
                        }
                    }
                }
            }
            w.stop();
            t
        };
        let light = total(&mut w, &near(Kind::Light, 77));
        let heavy = total(&mut w, &near(Kind::Heavy, 83));
        assert!(heavy * 10 < light * 8, "armour and glancing blows: {heavy} vs {light}");
    }

    #[test]
    fn a_passives_proc_casts_its_spell_at_most_once_a_second() {
        let b = book_plus();
        let mut c = mage(&b);
        c.mods.procs = vec![(1, 9999, 100)];
        let us = units(Kind::Target);
        let out = c.cast(0, here(), &us, &b, on(2136, DUMMY));
        let gos: Vec<u32> = out.iter().filter_map(|o| if let Out::Pkt(SMSG_SPELL_GO, x) = o { let mut at = 0; read_packed(x, &mut at); read_packed(x, &mut at); Some(u32::from_le_bytes(x[at + 1..at + 5].try_into().unwrap())) } else { None }).collect();
        assert_eq!(gos, vec![2136, 9999], "the proc's own go after the hit");
        assert!(out.iter().any(|o| matches!(o, Out::Event("proc", d) if d == "9999")));
        c.cooldowns.clear();
        c.gcd_until = 0;
        let again = c.cast(500, here(), &us, &b, on(2136, DUMMY));
        assert!(!again.iter().any(|o| matches!(o, Out::Event("proc", _))), "not again within a second");
        c.cooldowns.clear();
        c.gcd_until = 0;
        assert!(c.cast(1600, here(), &us, &b, on(2136, DUMMY)).iter().any(|o| matches!(o, Out::Event("proc", _))));
    }

    #[test]
    fn the_boss_casts_at_you_and_a_kick_stops_it() {
        let b = book_plus();
        let mut c = mage(&b);
        knows_too(&mut c, 2139);
        let boss = 0xF130_0000_0100_0999u64;
        let us = vec![Unit { guid: boss, at: At { x: 10.0, ..here() }, kind: Kind::Boss, level: 80 }];
        c.unit_health(&us[0]);
        c.hurt(0, &us, boss, 133, 100, 4, false, false);
        let mut cast_at = None;
        for t in (50..=BOSS_CAST_FIRST_MS + 100).step_by(50) {
            for o in c.tick(t, At { x: -30.0, ..here() }, &us, &b) {
                if let Out::Pkt(SMSG_SPELL_START, x) = &o {
                    let mut at = 0;
                    read_packed(x, &mut at);
                    read_packed(x, &mut at);
                    if u32::from_le_bytes(x[at + 1..at + 5].try_into().unwrap()) == BOSS_CAST_SPELL {
                        cast_at = Some(t);
                    }
                }
            }
        }
        assert!(cast_at.is_some() && c.boss.as_ref().unwrap().casting.is_some(), "the boss casts");
        // the kick: its cast fails, counted
        let out = c.cast(BOSS_CAST_FIRST_MS + 200, here(), &us, &b, on(2139, boss));
        assert!(ops(&out).contains(&SMSG_SPELL_FAILURE), "{out:?}");
        let f = body(&out, SMSG_SPELL_FAILURE);
        let mut at = 0;
        assert_eq!(read_packed(&f, &mut at), Some(boss));
        assert!(out.iter().any(|o| matches!(o, Out::Middle(m) if m == "Interrupted!")));
        assert_eq!((c.boss.as_ref().unwrap().kicked, c.boss.as_ref().unwrap().casts), (1, 1));
        // the next one lands: it hurts you
        let full = c.health;
        let mut landed = false;
        for t in (BOSS_CAST_FIRST_MS + 250..=BOSS_CAST_FIRST_MS + BOSS_CAST_EVERY_MS + BOSS_CAST_MS as u64 + 4000).step_by(50) {
            for o in c.tick(t, At { x: -30.0, ..here() }, &us, &b) {
                landed |= matches!(&o, Out::Middle(m) if m.starts_with("Its cast hit you"));
            }
        }
        assert!(landed && c.health < full, "{} of {full}", c.health);
        let max = c.hp[&boss].max;
        let out = c.hurt(60_000, &us, boss, 133, max, 4, false, false);
        assert!(out.iter().any(|o| matches!(o, Out::Boss { kicked: 1, casts: 2, .. })), "{out:?}");
        // invulnerable: nothing hurts
        c.god = true;
        let h = c.health;
        assert!(c.hurt_me(70_000, 5000).is_empty() && c.health == h);
    }

    #[test]
    fn healing_counts_on_its_own_meter_with_yourself_and_the_group() {
        let mut c = Combat::new(1);
        let b = Book::default();
        c.health = 50;
        c.heal(1000, 1, 2061, 30, false, None);
        assert_eq!((c.heals.total, c.meter.total), (30, 0));
        assert!(c.readout(false).contains(";hps=") && c.readout(false).contains("healed=30"));
        // the group: five friends, bleeding, the first more
        let us: Vec<Unit> = (0..5u64).map(|k| Unit { guid: DUMMY + k, at: At { x: 3.0, y: k as f32, ..here() }, kind: Kind::Group, level: 80 }).collect();
        for u in &us {
            let h = c.unit_health(u);
            assert_eq!(h.cur, h.max * 6 / 10);
        }
        assert!(!Kind::Group.hostile() && Kind::Group.faction() == 35);
        for t in (0..=5000).step_by(50) {
            c.tick(t, here(), &us, &b);
        }
        let first = c.hp[&DUMMY].cur;
        assert!(first < c.hp[&DUMMY].max * 6 / 10, "they lose health");
        assert!(us.iter().skip(1).all(|u| c.hp[&u.guid].cur > first), "the first (the tank) loses more");
        let out = c.heal(6000, DUMMY + 1, 2061, 1000, false, None);
        assert!(ops(&out).contains(&SMSG_SPELLHEALLOG) && c.heals.total == 1030);
        assert_eq!(c.reset_meters().len(), 1);
        assert_eq!(c.heals.total, 0);
        for k in [Kind::Light, Kind::Heavy, Kind::Group] {
            assert_eq!(Kind::from_word(k.word()), Some(k));
        }
        assert_eq!(Kind::Heavy.armor(), HEAVY_ARMOR);
    }

    #[test]
    fn a_practice_run_casts_the_bar_for_you_and_ends_with_its_numbers() {
        let b = book();
        let mut c = mage(&b);
        let us = units(Kind::Target);
        c.unit_health(&us[0]);
        c.start_demo(0, DUMMY, 20);
        let mut casts: Vec<u32> = Vec::new();
        let mut done = None;
        for t in (0..=21_000).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                match o {
                    Out::Pkt(SMSG_SPELL_START, x) => {
                        let mut at = 0;
                        read_packed(&x, &mut at);
                        read_packed(&x, &mut at);
                        casts.push(u32::from_le_bytes(x[at + 1..at + 5].try_into().unwrap()));
                    }
                    Out::Demo { secs, total, dps } => done = Some((secs, total, dps)),
                    _ => {}
                }
            }
        }
        assert!(casts.contains(&143) && casts.contains(&2136), "Fireball and Fire Blast when it's ready: {casts:?}");
        assert!(!casts.contains(&1459), "not the buff at a dummy");
        let (secs, total, dps) = done.expect("its numbers");
        assert_eq!(secs, 20.0);
        assert!(total > 5000 && dps == (total as f32 / 20.0).round() as u32, "{total} {dps}");
        assert!(c.demo.is_none());
    }
    #[test]
    fn a_pet_comes_follows_fights_your_target_and_goes() {
        let mut rows = fake::sample();
        rows.push(fake::Row { id: 688, name: "Summon Imp".into(), level: 1, cast_index: 1, school: 32, effects: vec![(56, 0, 1, 1, 0, 0, 0, 416)], ..Default::default() });
        rows.push(fake::Row { id: 2641, name: "Dismiss Pet".into(), level: 10, cast_index: 1, school: 1, effects: vec![(3, 0, 1, 1, 0, 0, 0, 0)], ..Default::default() });
        let b = Book::from_bytes(Some(&fake::spells(&rows)), Some(&fake::abilities(&fake::ABILITIES)), Some(&fake::four(&fake::CAST_TIMES)), Some(&fake::four(&fake::DURATIONS)), None);
        let mut c = mage(&b);
        knows_too(&mut c, 688);
        knows_too(&mut c, 2641);
        let us = units(Kind::Target);
        let out = c.cast(0, here(), &us, &b, on(688, 0));
        let pet = c.pet.clone().expect("a pet");
        assert_eq!((pet.def.name, pet.def.bolt), ("Imp", 47964));
        let create = out.iter().find_map(|o| match o { Out::Pkt(SMSG_UPDATE_OBJECT, x) if x.get(4) == Some(&2) => Some(x.clone()), _ => None }).expect("its create");
        let mut at = 5;
        assert_eq!(read_packed(&create, &mut at), Some(pet.guid));
        assert!(ops(&out).contains(&pets::SMSG_PET_SPELLS));
        // you fight the dummy: the imp casts its bolt at it, its damage on your meter
        c.attack(10, &us, DUMMY);
        let mut bolts = 0;
        for t in (50..=8000).step_by(50) {
            for o in c.tick(t, here(), &us, &b) {
                if let Out::Pkt(SMSG_SPELLNONMELEEDAMAGELOG, x) = o {
                    let mut at = 0;
                    read_packed(&x, &mut at);
                    if read_packed(&x, &mut at) == Some(pet.guid) {
                        bolts += 1;
                    }
                }
            }
        }
        assert!(bolts >= 2, "{bolts}");
        assert!(c.meter.total > 0);
        // stay, then follow: it walks after you
        let act = |a: u32, t: u32| {
            let mut v = pet.guid.to_le_bytes().to_vec();
            v.extend_from_slice(&(a | t << 24).to_le_bytes());
            v.extend_from_slice(&0u64.to_le_bytes());
            v
        };
        c.stop();
        c.last_target = 0;
        c.pet_action(&us, &act(1, 7));
        let far = At { x: -40.0, ..here() };
        let mut moved = false;
        for t in (9000..=12_000).step_by(50) {
            moved |= c.tick(t, far, &us, &b).iter().any(|o| matches!(o, Out::Pkt(pets::SMSG_MONSTER_MOVE, _)));
        }
        assert!(moved, "it follows");
        c.pet_action(&us, &act(0, 6));
        assert_eq!(c.pet.as_ref().unwrap().react, 0, "passive");
        // dismissed: gone, the bar too
        let out = c.cast(20_000, here(), &us, &b, on(2641, 0));
        assert!(c.pet.is_none() && ops(&out).contains(&pets::SMSG_DESTROY_OBJECT), "{out:?}");
        let out = c.summon(21_000, here(), 69);
        assert_eq!(c.pet.as_ref().map(|p| p.def.name), Some("Wolf"));
        assert!(ops(&out).contains(&pets::SMSG_PET_SPELLS));
        let wolf = c.pet.as_ref().unwrap().guid;
        let mut v = wolf.to_le_bytes().to_vec();
        v.extend_from_slice(&(3u32 | 7 << 24).to_le_bytes());
        assert_eq!(c.pet_action(&us, &v).len(), 4, "the bar's Dismiss");
        assert!(c.pet.is_none());
    }
}
