//! The player's own spell tables, for the sandbox combat (combat.rs) and the class picker (classes.rs).
//!
//! Like `models.rs`: these are the player's own game data, read from their own game archives by the launcher and put
//! in the Dens folder (`launcher/src-tauri/src/host/lite.rs` `game_tables`). Den Lite never ships them; with none
//! there, the class picker says so and only the auto attack works.
//!
//! The tables (3.3.5a, build 12340; wowdev wiki and AzerothCore's `DBCfmt.h`, checked against each file's own field
//! count and record size, a table with another shape is not used):
//! * `Spell.dbc` (234 fields, 936 bytes): the columns in [`col`].
//! * `SkillLineAbility.dbc` (14 fields): id, skill line, spell, race mask, class mask, ..., the next rank (8).
//! * `SpellCastTimes.dbc` (4): id, base ms, per level, minimum.
//! * `SpellDuration.dbc` (4): id, base ms (-1: until cancelled), per level, maximum.
//! * `SpellRadius.dbc` (4): id, radius, per level, maximum.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const SPELL: &str = "Spell.dbc";
pub const SKILL_LINE_ABILITY: &str = "SkillLineAbility.dbc";
pub const CAST_TIMES: &str = "SpellCastTimes.dbc";
pub const DURATIONS: &str = "SpellDuration.dbc";
pub const RADIUS: &str = "SpellRadius.dbc";
/// Every table the launcher copies for the combat (host/lite.rs `game_tables` has the same list).
pub const FILES: [&str; 5] = [SPELL, SKILL_LINE_ABILITY, CAST_TIMES, DURATIONS, RADIUS];

/// Spell.dbc's columns (0-based, 3.3.5a).
pub mod col {
    pub const ID: usize = 0;
    pub const ATTRIBUTES: usize = 4;
    pub const ATTRIBUTES_EX: usize = 5;
    pub const STANCES: usize = 12;
    pub const CAST_TIME_INDEX: usize = 28;
    pub const RECOVERY_TIME: usize = 29;
    pub const CATEGORY_RECOVERY_TIME: usize = 30;
    pub const INTERRUPT_FLAGS: usize = 31;
    pub const PROC_FLAGS: usize = 34;
    pub const PROC_CHANCE: usize = 35;
    pub const MAX_LEVEL: usize = 37;
    pub const BASE_LEVEL: usize = 38;
    pub const SPELL_LEVEL: usize = 39;
    pub const DURATION_INDEX: usize = 40;
    pub const POWER_TYPE: usize = 41;
    pub const MANA_COST: usize = 42;
    pub const MANA_COST_PER_LEVEL: usize = 43;
    pub const RANGE_INDEX: usize = 46;
    pub const SPEED: usize = 47;
    pub const EQUIPPED_ITEM_CLASS: usize = 68;
    pub const EFFECT: usize = 71;
    pub const EFFECT_DIE_SIDES: usize = 74;
    pub const EFFECT_REAL_POINTS_PER_LEVEL: usize = 77;
    pub const EFFECT_BASE_POINTS: usize = 80;
    pub const EFFECT_IMPLICIT_TARGET_A: usize = 86;
    pub const EFFECT_IMPLICIT_TARGET_B: usize = 89;
    pub const EFFECT_RADIUS_INDEX: usize = 92;
    pub const EFFECT_APPLY_AURA_NAME: usize = 95;
    pub const EFFECT_AMPLITUDE: usize = 98;
    pub const EFFECT_MISC_VALUE: usize = 110;
    pub const EFFECT_TRIGGER_SPELL: usize = 116;
    pub const SPELL_NAME: usize = 136;
    pub const RANK: usize = 153;
    pub const MANA_COST_PERCENTAGE: usize = 204;
    pub const START_RECOVERY_CATEGORY: usize = 205;
    pub const START_RECOVERY_TIME: usize = 206;
    pub const SPELL_FAMILY_NAME: usize = 208;
    pub const DMG_CLASS: usize = 213;
    pub const SCHOOL_MASK: usize = 225;
    pub const RUNE_COST_ID: usize = 226;
    pub const FIELDS: u32 = 234;
}

/// SPELL_ATTR0_PASSIVE.
pub const ATTR0_PASSIVE: u32 = 0x40;
/// SPELL_ATTR1_CHANNELED_1 / CHANNELED_2.
pub const ATTR1_CHANNELED: u32 = 0x4 | 0x40;

/// One spell effect (of three).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Effect {
    pub kind: u32,
    pub die: i32,
    pub per_level: f32,
    pub base: i32,
    pub target_a: u32,
    pub target_b: u32,
    pub radius: u32,
    pub aura: u32,
    pub amplitude: u32,
    pub misc: i32,
    pub trigger: u32,
}

/// What Den Lite uses of a spell.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spell {
    pub id: u32,
    pub name: String,
    pub rank: String,
    pub attributes: u32,
    pub attributes_ex: u32,
    pub stances: u32,
    pub cast_index: u32,
    pub recovery: u32,
    pub category_recovery: u32,
    pub interrupt_flags: u32,
    /// What makes a passive's proc fire (ProcFlags) and how often (per cent).
    pub proc_flags: u32,
    pub proc_chance: u32,
    pub max_level: u32,
    pub base_level: u32,
    pub spell_level: u32,
    pub duration_index: u32,
    pub power_type: i32,
    pub cost: u32,
    pub cost_per_level: u32,
    pub cost_pct: u32,
    pub range_index: u32,
    pub speed: f32,
    pub equipped_item_class: i32,
    pub effects: [Effect; 3],
    pub gcd: u32,
    pub gcd_category: u32,
    pub family: u32,
    pub dmg_class: u32,
    pub school_mask: u32,
    pub rune_cost: u32,
}

impl Spell {
    pub fn passive(&self) -> bool {
        self.attributes & ATTR0_PASSIVE != 0
    }
    pub fn channeled(&self) -> bool {
        self.attributes_ex & ATTR1_CHANNELED != 0
    }
    /// "Fireball (Rank 3)" or "Fireball".
    pub fn title(&self) -> String {
        if self.rank.is_empty() { self.name.clone() } else { format!("{} ({})", self.name, self.rank) }
    }
}

/// One SkillLineAbility row.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ability {
    pub skill: u32,
    pub spell: u32,
    pub race_mask: u32,
    pub class_mask: u32,
    pub next: u32,
}

/// The tables Den Lite read.
#[derive(Debug, Default)]
pub struct Book {
    pub spells: HashMap<u32, Spell>,
    pub abilities: Vec<Ability>,
    /// id -> (base, per level, minimum)
    pub cast_times: HashMap<u32, (i32, i32, i32)>,
    /// id -> (base, per level, maximum)
    pub durations: HashMap<u32, (i32, i32, i32)>,
    pub radius: HashMap<u32, f32>,
}

/// A WDBC table's records without copying them: (fields, record size, records, strings).
pub struct Table<'a> {
    pub fields: u32,
    pub size: usize,
    pub n: usize,
    rec: &'a [u8],
    strings: &'a [u8],
}

impl<'a> Table<'a> {
    pub fn parse(b: &'a [u8]) -> Option<Table<'a>> {
        if b.len() < 20 || &b[..4] != b"WDBC" {
            return None;
        }
        let u = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize;
        let (n, fields, size, sn) = (u(4), u(8) as u32, u(12), u(16));
        if size == 0 || size > 4096 || n > 4_000_000 || 20usize.checked_add(n.checked_mul(size)?)?.checked_add(sn)? != b.len() {
            return None;
        }
        Some(Table { fields, size, n, rec: &b[20..20 + n * size], strings: &b[20 + n * size..] })
    }
    pub fn row(&self, i: usize) -> &'a [u8] {
        &self.rec[i * self.size..(i + 1) * self.size]
    }
    pub fn u(r: &[u8], f: usize) -> u32 {
        r.get(f * 4..f * 4 + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0)
    }
    pub fn i(r: &[u8], f: usize) -> i32 {
        Self::u(r, f) as i32
    }
    pub fn f(r: &[u8], f: usize) -> f32 {
        f32::from_bits(Self::u(r, f))
    }
    pub fn s(&self, off: u32) -> String {
        let rest = self.strings.get(off as usize..).unwrap_or(&[]);
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        String::from_utf8_lossy(&rest[..end]).to_string()
    }
    /// The first locale's text that isn't empty (16 locales from `f`).
    pub fn text(&self, r: &[u8], f: usize) -> String {
        (0..16).map(|k| self.s(Self::u(r, f + k))).find(|t| !t.is_empty()).unwrap_or_default()
    }
}

pub fn spell_from_row(t: &Table, r: &[u8]) -> Spell {
    use col::*;
    let u = |f| Table::u(r, f);
    let i = |f| Table::i(r, f);
    let mut effects = [Effect::default(); 3];
    for (k, e) in effects.iter_mut().enumerate() {
        *e = Effect {
            kind: u(EFFECT + k),
            die: i(EFFECT_DIE_SIDES + k),
            per_level: Table::f(r, EFFECT_REAL_POINTS_PER_LEVEL + k),
            base: i(EFFECT_BASE_POINTS + k),
            target_a: u(EFFECT_IMPLICIT_TARGET_A + k),
            target_b: u(EFFECT_IMPLICIT_TARGET_B + k),
            radius: u(EFFECT_RADIUS_INDEX + k),
            aura: u(EFFECT_APPLY_AURA_NAME + k),
            amplitude: u(EFFECT_AMPLITUDE + k),
            misc: i(EFFECT_MISC_VALUE + k),
            trigger: u(EFFECT_TRIGGER_SPELL + k),
        };
    }
    Spell {
        id: u(ID),
        name: t.text(r, SPELL_NAME),
        rank: t.text(r, RANK),
        attributes: u(ATTRIBUTES),
        attributes_ex: u(ATTRIBUTES_EX),
        stances: u(STANCES),
        cast_index: u(CAST_TIME_INDEX),
        recovery: u(RECOVERY_TIME),
        category_recovery: u(CATEGORY_RECOVERY_TIME),
        interrupt_flags: u(INTERRUPT_FLAGS),
        proc_flags: u(PROC_FLAGS),
        proc_chance: u(PROC_CHANCE),
        max_level: u(MAX_LEVEL),
        base_level: u(BASE_LEVEL),
        spell_level: u(SPELL_LEVEL),
        duration_index: u(DURATION_INDEX),
        power_type: i(POWER_TYPE),
        cost: u(MANA_COST),
        cost_per_level: u(MANA_COST_PER_LEVEL),
        cost_pct: u(MANA_COST_PERCENTAGE),
        range_index: u(RANGE_INDEX),
        speed: Table::f(r, SPEED),
        equipped_item_class: i(EQUIPPED_ITEM_CLASS),
        effects,
        gcd: u(START_RECOVERY_TIME),
        gcd_category: u(START_RECOVERY_CATEGORY),
        family: u(SPELL_FAMILY_NAME),
        dmg_class: u(DMG_CLASS),
        school_mask: u(SCHOOL_MASK),
        rune_cost: u(RUNE_COST_ID),
    }
}

impl Book {
    /// The tables from their bytes (any of them may be missing: None there). Spell.dbc of another shape: no spells.
    pub fn from_bytes(spell: Option<&[u8]>, sla: Option<&[u8]>, casts: Option<&[u8]>, durs: Option<&[u8]>, radius: Option<&[u8]>) -> Book {
        let mut b = Book::default();
        if let Some(t) = spell.and_then(Table::parse).filter(|t| t.fields == col::FIELDS && t.size == col::FIELDS as usize * 4) {
            for k in 0..t.n {
                let s = spell_from_row(&t, t.row(k));
                b.spells.insert(s.id, s);
            }
        }
        if let Some(t) = sla.and_then(Table::parse).filter(|t| t.fields == 14 && t.size == 56) {
            for k in 0..t.n {
                let r = t.row(k);
                b.abilities.push(Ability { skill: Table::u(r, 1), spell: Table::u(r, 2), race_mask: Table::u(r, 3), class_mask: Table::u(r, 4), next: Table::u(r, 8) });
            }
        }
        if let Some(t) = casts.and_then(Table::parse).filter(|t| t.fields == 4 && t.size == 16) {
            for k in 0..t.n {
                let r = t.row(k);
                b.cast_times.insert(Table::u(r, 0), (Table::i(r, 1), Table::i(r, 2), Table::i(r, 3)));
            }
        }
        if let Some(t) = durs.and_then(Table::parse).filter(|t| t.fields == 4 && t.size == 16) {
            for k in 0..t.n {
                let r = t.row(k);
                b.durations.insert(Table::u(r, 0), (Table::i(r, 1), Table::i(r, 2), Table::i(r, 3)));
            }
        }
        if let Some(t) = radius.and_then(Table::parse).filter(|t| t.fields == 4 && t.size == 16) {
            for k in 0..t.n {
                let r = t.row(k);
                b.radius.insert(Table::u(r, 0), Table::f(r, 1));
            }
        }
        b
    }

    pub fn read(dir: &Path) -> Book {
        let get = |n: &str| std::fs::read(dir.join(n)).ok();
        let (s, a, c, d, r) = (get(SPELL), get(SKILL_LINE_ABILITY), get(CAST_TIMES), get(DURATIONS), get(RADIUS));
        Book::from_bytes(s.as_deref(), a.as_deref(), c.as_deref(), d.as_deref(), r.as_deref())
    }

    pub fn spell(&self, id: u32) -> Option<&Spell> {
        self.spells.get(&id)
    }

    /// The cast time in milliseconds at `level` (SpellCastTimes: base + per level, at least the minimum; 0: instant).
    pub fn cast_ms(&self, s: &Spell, level: u32) -> u32 {
        match self.cast_times.get(&s.cast_index) {
            Some(&(base, per, min)) => (base + per * level as i32).max(min).max(0) as u32,
            None => 0,
        }
    }

    /// The duration in milliseconds (SpellDuration; 0: none, u32::MAX: until cancelled).
    pub fn duration_ms(&self, s: &Spell, level: u32) -> u32 {
        match self.durations.get(&s.duration_index) {
            Some(&(base, _, _)) if base < 0 => u32::MAX,
            Some(&(base, per, max)) => {
                let d = base + per * level as i32;
                (if max > 0 { d.min(max) } else { d }).max(0) as u32
            }
            None => 0,
        }
    }

    /// An effect's radius in yards (SpellRadius; none: 0).
    pub fn radius(&self, e: &Effect) -> f32 {
        self.radius.get(&e.radius).copied().unwrap_or(0.0)
    }
}

/// Every mount the tables have (`.den mounts`): spells with SPELL_AURA_MOUNTED (78), not passive, with a name
/// (none of the game's own brand words: those are never shown here), sorted by name.
pub fn mounts(book: &Book) -> Vec<u32> {
    let mut v: Vec<&Spell> = book
        .spells
        .values()
        .filter(|s| s.effects.iter().any(|e| e.kind == 6 && e.aura == 78) && !s.passive() && !s.name.is_empty() && !crate::companion::branded(&s.name))
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    v.into_iter().map(|s| s.id).collect()
}

/// The tables, read once per Dens folder (Spell.dbc is large: kept for the program's life). None there yet: tried
/// again at most every 10 seconds (the launcher copies them as Den Lite starts).
static BOOK: Mutex<Option<(PathBuf, Arc<Book>, std::time::Instant)>> = Mutex::new(None);

pub fn book(dir: &Path) -> Arc<Book> {
    let mut g = BOOK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((d, b, at)) = g.as_ref() {
        if d == dir && (!b.spells.is_empty() || at.elapsed().as_secs() < 10) {
            return b.clone();
        }
    }
    let b = Arc::new(Book::read(dir));
    *g = Some((dir.to_path_buf(), b.clone(), std::time::Instant::now()));
    b
}

/// Forget the tables read (a test that writes new ones).
pub fn forget() {
    *BOOK.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Test helpers: tables written as the game's own are, from a few rows (the tests of classes.rs, combat.rs and the
/// headless client make their own small tables; nothing of the game's own is in this repository).
pub mod fake {
    use super::col;

    /// A WDBC file: rows of u32 fields, with their strings.
    pub fn table(fields: u32, rows: &[Vec<u32>], strings: &[u8]) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend_from_slice(b"WDBC");
        let s: Vec<u8> = if strings.is_empty() { vec![0] } else { strings.to_vec() };
        for v in [rows.len() as u32, fields, fields * 4, s.len() as u32] {
            o.extend_from_slice(&v.to_le_bytes());
        }
        for r in rows {
            for f in 0..fields as usize {
                o.extend_from_slice(&r.get(f).copied().unwrap_or(0).to_le_bytes());
            }
        }
        o.extend_from_slice(&s);
        o
    }

    /// A made-up spell row: what tests need set, the rest 0.
    #[derive(Clone, Default)]
    pub struct Row {
        pub id: u32,
        pub name: String,
        pub level: u32,
        pub cast_index: u32,
        pub recovery: u32,
        pub gcd: u32,
        pub power: i32,
        pub cost: u32,
        pub school: u32,
        pub attributes: u32,
        pub attributes_ex: u32,
        pub duration_index: u32,
        pub proc_chance: u32,
        /// (effect, base, die, target a, aura, amplitude, trigger, misc)
        pub effects: Vec<(u32, i32, i32, u32, u32, u32, u32, i32)>,
    }

    /// Spell.dbc from rows.
    pub fn spells(rows: &[Row]) -> Vec<u8> {
        let mut strings = vec![0u8];
        let mut out = Vec::new();
        for r in rows {
            let mut v = vec![0u32; col::FIELDS as usize];
            v[col::ID] = r.id;
            v[col::SPELL_LEVEL] = r.level;
            v[col::BASE_LEVEL] = r.level;
            v[col::CAST_TIME_INDEX] = r.cast_index;
            v[col::RECOVERY_TIME] = r.recovery;
            v[col::START_RECOVERY_TIME] = r.gcd;
            v[col::POWER_TYPE] = r.power as u32;
            v[col::MANA_COST] = r.cost;
            v[col::SCHOOL_MASK] = r.school;
            v[col::ATTRIBUTES] = r.attributes;
            v[col::ATTRIBUTES_EX] = r.attributes_ex;
            v[col::DURATION_INDEX] = r.duration_index;
            v[col::PROC_CHANCE] = r.proc_chance;
            v[col::PROC_FLAGS] = if r.proc_chance > 0 { 0x0001_0014 } else { 0 };
            for (k, e) in r.effects.iter().enumerate().take(3) {
                v[col::EFFECT + k] = e.0;
                v[col::EFFECT_BASE_POINTS + k] = e.1 as u32;
                v[col::EFFECT_DIE_SIDES + k] = e.2 as u32;
                v[col::EFFECT_IMPLICIT_TARGET_A + k] = e.3;
                v[col::EFFECT_APPLY_AURA_NAME + k] = e.4;
                v[col::EFFECT_AMPLITUDE + k] = e.5;
                v[col::EFFECT_TRIGGER_SPELL + k] = e.6;
                v[col::EFFECT_MISC_VALUE + k] = e.7 as u32;
            }
            v[col::SPELL_NAME] = strings.len() as u32;
            strings.extend_from_slice(r.name.as_bytes());
            strings.push(0);
            out.push(v);
        }
        table(col::FIELDS, &out, &strings)
    }

    /// SkillLineAbility.dbc from (skill, spell, class mask, next rank).
    pub fn abilities(rows: &[(u32, u32, u32, u32)]) -> Vec<u8> {
        let v: Vec<Vec<u32>> = rows.iter().enumerate().map(|(i, &(skill, spell, cm, next))| vec![i as u32 + 1, skill, spell, 0, cm, 0, 0, 0, next]).collect();
        table(14, &v, &[])
    }

    /// A four-field table (cast times, durations): id, a, b, c.
    pub fn four(rows: &[(u32, i32, i32, i32)]) -> Vec<u8> {
        let v: Vec<Vec<u32>> = rows.iter().map(|&(id, a, b, c)| vec![id, a as u32, b as u32, c as u32]).collect();
        table(4, &v, &[])
    }

    /// The sample's tables in one book (no files).
    pub fn book() -> super::Book {
        let (s, a, c, d) = (spells(&sample()), abilities(&ABILITIES), four(&CAST_TIMES), four(&DURATIONS));
        super::Book::from_bytes(Some(&s), Some(&a), Some(&c), Some(&d), None)
    }

    pub const ABILITIES: [(u32, u32, u32, u32); 6] = [(8, 133, 128, 143), (8, 143, 128, 0), (8, 2136, 128, 0), (237, 1459, 128, 0), (237, 5143, 128, 0), (26, 78, 1, 0)];
    pub const CAST_TIMES: [(u32, i32, i32, i32); 2] = [(1, 0, 0, 0), (5, 2500, 0, 2500)];
    pub const DURATIONS: [(u32, i32, i32, i32); 4] = [(1, 10000, 0, 10000), (5, 3000, 0, 3000), (21, -1, 0, -1), (9, 1_800_000, 0, 1_800_000)];

    /// Every table in a folder: a mage with Fireball (cast 2.5 s, rank 1 -> 2), Fire Blast (instant, 8 s cooldown),
    /// Arcane Intellect (a buff on self), Arcane Missiles (channelled, triggering a missile), a warrior's strike, and
    /// the cast times and durations they use. The tests' world.
    pub fn write_all(dir: &std::path::Path) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(super::SPELL), spells(&sample()));
        let _ = std::fs::write(dir.join(super::SKILL_LINE_ABILITY), abilities(&ABILITIES));
        let _ = std::fs::write(dir.join(super::CAST_TIMES), four(&CAST_TIMES));
        let _ = std::fs::write(dir.join(super::DURATIONS), four(&DURATIONS));
        let _ = std::fs::write(dir.join(super::RADIUS), four(&[]));
    }

    pub fn sample() -> Vec<Row> {
        vec![
            Row { id: 133, name: "Fireball".into(), level: 1, cast_index: 5, gcd: 1500, power: 0, cost: 30, school: 4,
                effects: vec![(2, 13, 9, 6, 0, 0, 0, 0)], ..Default::default() },
            Row { id: 143, name: "Fireball".into(), level: 6, cast_index: 5, gcd: 1500, power: 0, cost: 45, school: 4,
                effects: vec![(2, 30, 11, 6, 0, 0, 0, 0)], ..Default::default() },
            Row { id: 2136, name: "Fire Blast".into(), level: 6, cast_index: 1, recovery: 8000, gcd: 1500, power: 0, cost: 40, school: 4,
                effects: vec![(2, 26, 9, 6, 0, 0, 0, 0)], ..Default::default() },
            Row { id: 1459, name: "Arcane Intellect".into(), level: 1, cast_index: 1, gcd: 1500, power: 0, cost: 60, school: 64, duration_index: 9,
                effects: vec![(6, 1, 1, 21, 29, 0, 0, 3)], ..Default::default() },
            Row { id: 5143, name: "Arcane Missiles".into(), level: 8, cast_index: 1, gcd: 1500, power: 0, cost: 85, school: 64, duration_index: 5,
                attributes_ex: 0x4, effects: vec![(6, 0, 1, 6, 23, 1000, 7268, 0)], ..Default::default() },
            Row { id: 7268, name: "Arcane Missile".into(), level: 8, cast_index: 1, power: 0, school: 64,
                effects: vec![(2, 23, 1, 6, 0, 0, 0, 0)], ..Default::default() },
            Row { id: 78, name: "Heroic Strike".into(), level: 1, cast_index: 1, power: 1, cost: 150, school: 1,
                effects: vec![(58, 10, 1, 6, 0, 0, 0, 0)], ..Default::default() },
            // a mount: the creature 3 (its model in creatures.rs), 100 % faster
            Row { id: 458, name: "Brown Horse".into(), level: 1, cast_index: 1, power: 0, school: 1,
                effects: vec![(6, 0, 1, 1, 78, 0, 0, 3), (6, 99, 1, 1, 32, 0, 0, 0)], ..Default::default() },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_read_back_as_written() {
        let rows = fake::sample();
        let b = Book::from_bytes(Some(&fake::spells(&rows)), Some(&fake::abilities(&[(8, 133, 128, 143)])), Some(&fake::four(&[(5, 2500, 0, 2500)])), Some(&fake::four(&[(21, -1, 0, -1), (5, 3000, 0, 3000)])), None);
        assert_eq!(b.spells.len(), rows.len());
        let fb = b.spell(133).unwrap();
        assert_eq!(fb.name, "Fireball");
        assert_eq!(fb.effects[0].kind, 2);
        assert_eq!((fb.effects[0].base, fb.effects[0].die, fb.effects[0].target_a), (13, 9, 6));
        assert_eq!((fb.cost, fb.power_type, fb.school_mask, fb.gcd), (30, 0, 4, 1500));
        assert_eq!(b.cast_ms(fb, 80), 2500);
        assert_eq!(b.cast_ms(b.spell(2136).unwrap(), 80), 0, "no cast time row: instant");
        assert_eq!(b.abilities, vec![Ability { skill: 8, spell: 133, race_mask: 0, class_mask: 128, next: 143 }]);
        assert!(b.spell(5143).unwrap().channeled() && !fb.channeled());
        assert_eq!(b.duration_ms(b.spell(5143).unwrap(), 80), 3000);
        let mut forever = fb.clone();
        forever.duration_index = 21;
        assert_eq!(b.duration_ms(&forever, 80), u32::MAX);
    }

    #[test]
    fn a_table_of_another_shape_is_not_used() {
        let wrong = fake::table(233, &[vec![133]], &[]);
        assert!(Book::from_bytes(Some(&wrong), None, None, None, None).spells.is_empty());
        assert!(Table::parse(b"WDBC\x01\0\0\0").is_none());
        let mut cut = fake::spells(&fake::sample());
        cut.pop();
        assert!(Book::from_bytes(Some(&cut), None, None, None, None).spells.is_empty(), "sizes that don't add up");
    }
}
