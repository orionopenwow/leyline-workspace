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

/// The game's messages read here (world.rs takes them before the message crate does).
pub const CLIENT_OPS: [u32; 8] =
    [CMSG_CAST_SPELL, CMSG_CANCEL_CAST, CMSG_CANCEL_AURA, CMSG_CANCEL_CHANNELLING, CMSG_SET_SELECTION, CMSG_ATTACKSWING, CMSG_ATTACKSTOP, classes::CMSG_SET_ACTION_BUTTON];
/// Every message this module writes (the headless test client reads them by hand).
pub const SERVER_OPS: [u16; 20] = [
    SMSG_CAST_FAILED, SMSG_SPELL_START, SMSG_SPELL_GO, SMSG_SPELL_FAILURE, SMSG_SPELL_COOLDOWN, MSG_CHANNEL_START, MSG_CHANNEL_UPDATE,
    SMSG_ATTACKSTART, SMSG_ATTACKSTOP, SMSG_ATTACKSWING_NOTINRANGE, SMSG_ATTACKSWING_CANT_ATTACK, SMSG_ATTACKERSTATEUPDATE, SMSG_SPELLHEALLOG,
    SMSG_CLEAR_COOLDOWN, SMSG_PERIODICAURALOG, SMSG_SPELLNONMELEEDAMAGELOG, SMSG_SPELL_FAILED_OTHER, SMSG_UPDATE_COMBO_POINTS, SMSG_AURA_UPDATE,
    classes::SMSG_INITIAL_SPELLS,
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
    /// The practice boss: dies, with timed circles to move out of.
    Boss,
    /// The companion: friendly, never hurt.
    Friend,
}

impl Kind {
    pub fn hostile(self) -> bool {
        !matches!(self, Kind::Healer | Kind::Friend)
    }
    /// FactionTemplate: 7 (neutral to all: yellow, attackable), 14 (hostile: the ones that hit back), 35 (friendly).
    pub fn faction(self) -> u32 {
        match self {
            Kind::Tank | Kind::Boss => 14,
            Kind::Healer | Kind::Friend => 35,
            _ => 7,
        }
    }
    pub fn max_health(self, level: u32) -> u32 {
        match self {
            Kind::Target | Kind::Mover | Kind::Tank => 5_000_000,
            Kind::Pack => 1_000_000,
            Kind::Healer => 20_000 + 250 * level,
            Kind::Boss => 1500 * level.max(1) + 30_000,
            Kind::Friend => 100,
        }
    }
    /// Health when it goes up (the healing dummy starts hurt).
    pub fn start_health(self, level: u32) -> u32 {
        match self {
            Kind::Healer => self.max_health(level) * 3 / 10,
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
        }
    }
    pub fn from_word(w: &str) -> Option<Kind> {
        Some(match w {
            "target" | "targets" | "single" | "dummy" | "dummies" => Kind::Target,
            "pack" | "aoe" | "group" => Kind::Pack,
            "mover" | "moving" | "walker" => Kind::Mover,
            "healer" | "heal" | "healing" | "wounded" => Kind::Healer,
            "tank" | "defence" | "defense" | "hitter" => Kind::Tank,
            "boss" => Kind::Boss,
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
    /// The boss is down (the best time per class).
    Boss { secs: f32, hits: u32 },
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
    let mut b = Vec::with_capacity(48);
    let hit: u32 = 0x2 | if crit { 0x80 } else { 0 };
    b.extend_from_slice(&hit.to_le_bytes());
    packed_guid(&mut b, attacker);
    packed_guid(&mut b, victim);
    b.extend_from_slice(&damage.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // overkill
    b.push(1);
    b.extend_from_slice(&1u32.to_le_bytes()); // physical
    b.extend_from_slice(&(damage as f32).to_le_bytes());
    b.extend_from_slice(&damage.to_le_bytes());
    b.push(1); // VICTIMSTATE_HIT
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
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
    let mut b = Vec::with_capacity(64 + hits.len() * 8);
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
    b.push(0);
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

    fn crit(&mut self) -> bool {
        self.roll() % 100 < 10
    }

    /// A class chosen (or none): full health and power, nothing going on.
    pub fn set_kit(&mut self, kit: Option<Kit>) {
        self.stats = kit.as_ref().map(|k| k.stats).unwrap_or_else(plain_stats);
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
        self.boss = None;
        self.parse = None;
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
        match &self.kit {
            Some(k) => k.spells.binary_search(&spell).is_ok(),
            None => spell == classes::ATTACK,
        }
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
        self.gos = self.gos.wrapping_add(1);
        out.push(Out::Pkt(SMSG_SPELL_GO, go_body(self.me, &r, now as u32, &hits, Some(self.power as u32))));
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
                    let dmg = (dmg.max(1) as f32 * if crit { 1.5 } else { 1.0 }) as u32;
                    out.extend(self.hurt(now, units, g, s.id, dmg, s.school_mask, crit, false));
                } else if is_heal(e) {
                    let amount = match e.kind {
                        136 => self.max_of(g) * value.max(0) as u32 / 100,
                        67 => self.max_of(g),
                        _ => (value + (self.stats.spell_power as f32 * coefficient(cast_ms) * 1.88) as i32).max(1) as u32,
                    };
                    let crit = self.crit();
                    let amount = if crit { amount * 3 / 2 } else { amount };
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
                    out.extend(self.aura_on(now, book, &s, e, k, g, value, cast_ms));
                }
            }
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
        // a training dummy never falls: refilled at a tenth
        if !boss && h.kind != Kind::Healer && h.cur < h.max / 10 {
            h.cur = h.max;
        }
        let (cur, max) = (h.cur, h.max);
        out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, cur)])));
        let healing = u.kind == Kind::Healer;
        let amount = delta.unsigned_abs();
        if self.meter.total == 0 || self.meter.healing != healing {
            self.meter = Meter { first: now, last: now, total: 0, healing, name: dummy_name(u.kind).into(), shown: 0 };
        }
        self.meter.total += amount;
        self.meter.last = now;
        if now >= self.meter.shown + READOUT_MS {
            self.meter.shown = now;
            out.push(Out::Dps(self.meter.line(false)));
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
                self.boss = Some(BossFight { guid: u.guid, start: now, next_circle: now + BOSS_EVERY_MS / 2, circle: None, hits: 0, count: 0 });
                out.push(Out::Middle("The boss turns to you. Watch the ground!".into()));
            }
            if cur == 0 {
                let b = self.boss.take().unwrap_or(BossFight { guid: u.guid, start: now, next_circle: 0, circle: None, hits: 0, count: 0 });
                let secs = (now.saturating_sub(b.start)) as f32 / 1000.0;
                if self.swing.is_some_and(|s| s.0 == u.guid) {
                    out.extend(self.stop());
                }
                // UNIT_DYNFLAG_DEAD (0x20): the game shows it fallen
                out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(u.guid, &[(field::HEALTH, 0), (field::DYNAMIC_FLAGS, 0x20)])));
                out.push(Out::Boss { secs, hits: b.hits });
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
        } else if let Some(h) = self.hp.get(&g).copied() {
            let u = Unit { guid: g, at: At::default(), kind: h.kind, level: 1 };
            if done > 0 {
                out.extend(self.took(now, &u, -(done as i64)));
            }
        }
        out
    }

    fn free_slot(&self, unit: u64) -> u8 {
        (0u8..56).find(|s| !self.auras.iter().any(|a| a.unit == unit && a.slot == *s)).unwrap_or(55)
    }

    /// An aura: shown on the unit with its duration; damage or healing over time ticks from the tick; a stance or
    /// form sets the shapeshift byte (UNIT_FIELD_BYTES_2's fourth: the stance bar's own).
    #[allow(clippy::too_many_arguments)]
    fn aura_on(&mut self, now: u64, book: &Book, s: &Spell, e: &Effect, k: usize, g: u64, value: i32, cast_ms: u32) -> Vec<Out> {
        let mut out = Vec::new();
        let dur = book.duration_ms(s, self.stats.level);
        let ends = if dur == 0 || dur == u32::MAX { 0 } else { now + dur as u64 };
        // the same spell again on the same unit: refreshed in its slot (a stance: switched off)
        let same = self.auras.iter().position(|a| a.unit == g && a.spell == s.id);
        let form = if e.aura == 36 { e.misc.clamp(0, 255) as u8 } else { 0 };
        if let Some(i) = same {
            if form != 0 && ends == 0 {
                return self.drop_aura(i);
            }
            let slot = self.auras[i].slot;
            self.auras[i].ends = ends;
            out.push(Out::Pkt(SMSG_AURA_UPDATE, aura_body(g, slot, s.id, self.me, 1 << k, !harmful(s), self.stats.level as u8, (ends > 0).then_some((dur, dur)))));
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
        let slot = self.free_slot(g);
        let every = if e.amplitude > 0 { e.amplitude as u64 } else { 3000 };
        let ticks = if ends > 0 { (dur as u64 / every).max(1) as u32 } else { 1 };
        let bonus = self.stats.spell_power as f32 * (dur.min(15_000) as f32 / 15_000.0).max(coefficient(cast_ms) * 0.5);
        let (amount, heal) = if is_dot(e) {
            ((value.max(1) as u32) + (bonus as u32 / ticks), false)
        } else if is_hot(e) {
            ((value.max(1) as u32) + (bonus as u32 * 2 / ticks), true)
        } else {
            (0, false)
        };
        self.auras.push(Aura { unit: g, slot, spell: s.id, ends, every, next: now + every, amount, heal, aura: e.aura, school: s.school_mask, form });
        out.push(Out::Pkt(SMSG_AURA_UPDATE, aura_body(g, slot, s.id, self.me, 1 << k, !harmful(s), self.stats.level as u8, (ends > 0).then_some((dur, dur)))));
        out
    }

    fn drop_aura(&mut self, i: usize) -> Vec<Out> {
        let a = self.auras.remove(i);
        let mut out = vec![Out::Pkt(SMSG_AURA_UPDATE, aura_body(a.unit, a.slot, 0, self.me, 0, true, 0, None))];
        if a.form != 0 && a.unit == self.me {
            self.form = 0;
            out.push(Out::Pkt(SMSG_UPDATE_OBJECT, values_body(self.me, &[(field::BYTES_2, 0)])));
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
                            let dmg = weapon_hit(&self.stats, r) * if crit { 2 } else { 1 };
                            if ranged {
                                let req = CastReq { count: 0, spell: classes::AUTO_SHOT, mask: TF_UNIT, unit: Some(g), src: None, dest: None };
                                out.push(Out::Pkt(SMSG_SPELL_GO, go_body(self.me, &req, now as u32, &[g], None)));
                                out.extend(self.hurt(now, units, g, classes::AUTO_SHOT, dmg, 1, crit, false));
                            } else {
                                out.push(Out::Pkt(SMSG_ATTACKERSTATEUPDATE, swing_body(self.me, g, dmg, crit)));
                                out.extend(self.took(now, &u, dmg as i64));
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
        // the fight is over: its numbers
        if self.meter.total > 0 && now >= self.meter.last + IDLE_MS && self.parse.as_ref().map_or(true, |p| p.start == 0) {
            let m = std::mem::take(&mut self.meter);
            let what = if m.healing { "healing" } else { "damage" };
            out.push(Out::Dps(m.line(true)));
            out.push(Out::Say(format!("{} {what} in {:.1} s: {} per second.", m.total, m.secs(), m.per_sec())));
            out.push(Out::Event("dps", m.line(true)));
        }
        if let Some(p) = self.parse.clone() {
            if p.start > 0 && now >= p.start + p.minutes as u64 * 60_000 {
                self.parse = None;
                let secs = p.minutes as f32 * 60.0;
                let dps = (p.total as f32 / secs).round() as u32;
                out.push(Out::Parse { minutes: p.minutes, dps, total: p.total, secs });
                out.extend(self.stop());
                self.meter = Meter::default();
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
            let hits_back = u.kind == Kind::Tank || (self.fight && u.kind.hostile() && u.kind != Kind::Boss);
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
            } else if now >= b.next_circle {
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
            }
            self.boss = Some(b);
        }
        out
    }

    /// The player is hurt (a dummy that hits back, the boss's circle with fight on): never below 1, back to full.
    fn hurt_me(&mut self, now: u64, dmg: u32) -> Vec<Out> {
        self.last_hurt = now;
        let mut out = Vec::new();
        if dmg >= self.health {
            self.health = self.stats.health;
            out.push(Out::Middle("That would have been the end of you. Back to full.".into()));
        } else {
            self.health -= dmg;
        }
        out.push(self.me_update());
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
    use std::collections::BTreeMap;

    fn book() -> Book {
        fake::book()
    }

    fn mage(b: &Book) -> Combat {
        let mut c = Combat::new(1);
        c.seed(7);
        c.set_kit(classes::kit(b, &Pick { class: 8, level: 80, buttons: BTreeMap::new() }));
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
        c.set_kit(classes::kit(&b, &Pick { class: 1, level: 80, buttons: BTreeMap::new() }));
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
        assert!(c.meter.healing && c.meter.total == 500);
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
}
