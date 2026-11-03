//! The class picker (owner 3 Nov: "allow class picker, any spell"): `.den class <class> [level]` makes the
//! ready-made character that class at that level (80 by default), with every spell the class learns up to it, from
//! the player's own Spell.dbc and SkillLineAbility.dbc (spells.rs), and an action bar of the class's main spells.
//!
//! What AzerothCore sends a level-80 character (Player::SendInitialPacketsBeforeAddToMap): SMSG_INITIAL_SPELLS with
//! every spell it knows (u8 0, u16 count, per spell u32 id and u16 0, u16 cooldown count) and SMSG_ACTION_BUTTONS
//! (u8 0 "initial", 144 buttons, each u32 action | type << 24; type 0 a spell). Its spells come from trainers and
//! talents; here: every spell of the class's skill lines (class mask, the highest rank the level allows), talents'
//! spells too (no talent points: the abilities only; their passive effects don't apply), plus the auto attack.
//!
//! The character stays human (the look and the character list as before); the class, level, health and power (mana,
//! rage, energy or runic power) are the class's, with simple numbers by level ([`stats`]). The choice is kept in the
//! Dens folder ([`FILE`]) with the player's own changes to the bar, per class.

use crate::spells::{Book, Spell};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub const FILE: &str = "den-lite.class.json";
/// The auto attack (every class) and the hunter's ranged auto attack.
pub const ATTACK: u32 = 6603;
pub const AUTO_SHOT: u32 = 75;
pub const MAX_LEVEL: u32 = 80;
/// Power types (UNIT_FIELD_BYTES_0's fourth byte; the power fields are 1 + this).
pub const MANA: u8 = 0;
pub const RAGE: u8 = 1;
pub const ENERGY: u8 = 3;
pub const RUNIC: u8 = 6;
/// The human race's bit in a race mask (spells for some races only).
const HUMAN_MASK: u32 = 1;
/// The ready-made character's race.
pub const RACE: u8 = 1;

pub const SMSG_ACTION_BUTTONS: u16 = 0x129;
pub const SMSG_INITIAL_SPELLS: u16 = 0x12A;
pub const CMSG_SET_ACTION_BUTTON: u32 = 0x128;
pub const BUTTONS: usize = 144;

/// A class: its id (ChrClasses), the word for `.den class`, its name, power, skill lines (SkillLine ids of its three
/// trees and its own lines), whether it fights in melee, and the main bar's spells by name (12 at most).
#[derive(Debug)]
pub struct ClassDef {
    pub id: u8,
    pub word: &'static str,
    pub name: &'static str,
    pub power: u8,
    pub lines: &'static [u32],
    pub melee: bool,
    pub bar: &'static [&'static str],
}

pub const CLASSES: &[ClassDef] = &[
    ClassDef { id: 1, word: "warrior", name: "Warrior", power: RAGE, lines: &[26, 256, 257], melee: true,
        bar: &["Attack", "Heroic Strike", "Mortal Strike", "Bloodthirst", "Execute", "Whirlwind", "Rend", "Thunder Clap", "Cleave", "Overpower", "Shield Slam", "Battle Shout"] },
    ClassDef { id: 2, word: "paladin", name: "Paladin", power: MANA, lines: &[594, 267, 184], melee: true,
        bar: &["Attack", "Crusader Strike", "Judgement of Light", "Divine Storm", "Consecration", "Exorcism", "Hammer of Wrath", "Holy Shock", "Holy Light", "Flash of Light", "Seal of Righteousness", "Blessing of Might"] },
    ClassDef { id: 3, word: "hunter", name: "Hunter", power: MANA, lines: &[50, 163, 51], melee: false,
        bar: &["Auto Shot", "Steady Shot", "Arcane Shot", "Multi-Shot", "Serpent Sting", "Aimed Shot", "Kill Shot", "Explosive Shot", "Raptor Strike", "Hunter's Mark", "Volley", "Aspect of the Hawk"] },
    ClassDef { id: 4, word: "rogue", name: "Rogue", power: ENERGY, lines: &[253, 38, 39], melee: true,
        bar: &["Attack", "Sinister Strike", "Eviscerate", "Slice and Dice", "Backstab", "Mutilate", "Rupture", "Envenom", "Hemorrhage", "Fan of Knives", "Kick", "Stealth"] },
    ClassDef { id: 5, word: "priest", name: "Priest", power: MANA, lines: &[613, 56, 78], melee: false,
        bar: &["Smite", "Shadow Word: Pain", "Mind Blast", "Mind Flay", "Holy Fire", "Penance", "Flash Heal", "Greater Heal", "Renew", "Power Word: Shield", "Prayer of Healing", "Power Word: Fortitude"] },
    ClassDef { id: 6, word: "deathknight", name: "Death Knight", power: RUNIC, lines: &[770, 771, 772, 776], melee: true,
        bar: &["Attack", "Icy Touch", "Plague Strike", "Blood Strike", "Death Strike", "Obliterate", "Heart Strike", "Scourge Strike", "Death Coil", "Howling Blast", "Blood Boil", "Horn of Winter"] },
    ClassDef { id: 7, word: "shaman", name: "Shaman", power: MANA, lines: &[375, 373, 374], melee: false,
        bar: &["Lightning Bolt", "Chain Lightning", "Earth Shock", "Flame Shock", "Lava Burst", "Stormstrike", "Lava Lash", "Healing Wave", "Lesser Healing Wave", "Chain Heal", "Riptide", "Lightning Shield"] },
    ClassDef { id: 8, word: "mage", name: "Mage", power: MANA, lines: &[237, 8, 6], melee: false,
        bar: &["Fireball", "Frostbolt", "Arcane Missiles", "Fire Blast", "Arcane Blast", "Pyroblast", "Ice Lance", "Arcane Explosion", "Frost Nova", "Cone of Cold", "Flamestrike", "Arcane Intellect"] },
    ClassDef { id: 9, word: "warlock", name: "Warlock", power: MANA, lines: &[355, 354, 593], melee: false,
        bar: &["Shadow Bolt", "Corruption", "Immolate", "Incinerate", "Curse of Agony", "Drain Life", "Chaos Bolt", "Haunt", "Unstable Affliction", "Conflagrate", "Rain of Fire", "Life Tap"] },
    ClassDef { id: 11, word: "druid", name: "Druid", power: MANA, lines: &[574, 134, 573], melee: false,
        bar: &["Wrath", "Starfire", "Moonfire", "Insect Swarm", "Starfall", "Hurricane", "Healing Touch", "Rejuvenation", "Regrowth", "Lifebloom", "Nourish", "Mark of the Wild"] },
];

/// `.den class <word>`: the class's word, its name, or a short form (dk, lock, pally...).
pub fn by_word(w: &str) -> Option<&'static ClassDef> {
    let w: String = w.to_lowercase().chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let w = match w.as_str() {
        "dk" | "death" | "deathknights" => "deathknight",
        "pally" | "pal" => "paladin",
        "lock" => "warlock",
        "sham" => "shaman",
        "war" | "warr" => "warrior",
        "hunt" => "hunter",
        other => other,
    };
    CLASSES.iter().find(|c| c.word == w || c.name.to_lowercase().replace(' ', "") == w || format!("{}s", c.word) == w)
}

pub fn by_id(id: u8) -> Option<&'static ClassDef> {
    CLASSES.iter().find(|c| c.id == id)
}

/// The class's bit in a class mask.
pub fn class_bit(id: u8) -> u32 {
    1u32 << (id.max(1) - 1)
}

/// `.den class <class> [level]`'s level: 1 to 80 (none: 80).
pub fn level_arg(s: &str) -> Result<u32, String> {
    if s.is_empty() {
        return Ok(MAX_LEVEL);
    }
    match s.parse::<u32>() {
        Ok(n) if (1..=MAX_LEVEL).contains(&n) => Ok(n),
        _ => Err(format!("Levels are 1 to {MAX_LEVEL}: .den class mage 80")),
    }
}

/// What the character is in numbers (simple, by level: no gear, no talents).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub level: u32,
    pub health: u32,
    pub power: u8,
    pub max_power: u32,
    pub attack_ms: u32,
    pub min_dmg: f32,
    pub max_dmg: f32,
    pub attack_power: u32,
    pub spell_power: u32,
}

/// The numbers for a class at a level. Health 100 + 2.5 level² (the plate classes a fifth more), mana 100 + 1.8
/// level², rage and runic power 100 (shown; 1000 in the field), energy 100; a weapon swing every 2.6 s (melee) or 2 s,
/// its damage by level, attack power 30 a level (melee) or 10, spell power 15 a level (casters) or 5.
pub fn stats(c: &ClassDef, level: u32) -> Stats {
    let l = level.clamp(1, MAX_LEVEL) as f32;
    let plate = matches!(c.id, 1 | 2 | 6);
    let health = ((100.0 + 2.5 * l * l) * if plate { 1.2 } else { 1.0 }).round() as u32;
    let max_power = match c.power {
        MANA => (100.0 + 1.8 * l * l).round() as u32,
        RAGE | RUNIC => 1000,
        _ => 100,
    };
    let attack_ms = if c.melee { 2600 } else { 2000 };
    let scale = attack_ms as f32 / 2000.0;
    Stats {
        level: level.clamp(1, MAX_LEVEL),
        health,
        power: c.power,
        max_power,
        attack_ms,
        min_dmg: (l * 5.0 * scale).max(1.0),
        max_dmg: (l * 7.5 * scale).max(2.0),
        attack_power: if c.melee || c.id == 3 { 30 * level } else { 10 * level },
        spell_power: if c.melee || c.id == 3 { 5 * level } else { 15 * level },
    }
}

/// Every spell the class knows at `level` (sorted): the abilities of its skill lines for its class (or any class)
/// and the human race, each at the highest rank the level allows (SkillLineAbility's next rank, else the same name's
/// highest level), and the auto attack (and the hunter's Auto Shot).
pub fn learn(book: &Book, c: &ClassDef, level: u32) -> Vec<u32> {
    let bit = class_bit(c.id);
    let lines: HashSet<u32> = c.lines.iter().copied().collect();
    let ok = |s: &Spell| s.spell_level <= level && !s.name.is_empty();
    let mut next: HashMap<u32, u32> = HashMap::new();
    let mut have: HashSet<u32> = HashSet::new();
    for a in &book.abilities {
        if !lines.contains(&a.skill) || (a.class_mask != 0 && a.class_mask & bit == 0) || (a.race_mask != 0 && a.race_mask & HUMAN_MASK == 0) {
            continue;
        }
        if a.next != 0 {
            next.insert(a.spell, a.next);
        }
        if book.spell(a.spell).is_some_and(ok) {
            have.insert(a.spell);
        }
    }
    // a rank whose next rank is known too is left out
    let mut keep: Vec<u32> = have.iter().copied().filter(|s| !next.get(s).is_some_and(|n| have.contains(n))).collect();
    // ranks the table doesn't link: the same name, only the highest level (spells with a rank only)
    let mut best: HashMap<String, (u32, u32)> = HashMap::new();
    for &id in &keep {
        let s = &book.spells[&id];
        if s.rank.starts_with("Rank") {
            let e = best.entry(s.name.clone()).or_insert((s.spell_level, id));
            if s.spell_level > e.0 || (s.spell_level == e.0 && id > e.1) {
                *e = (s.spell_level, id);
            }
        }
    }
    keep.retain(|id| {
        let s = &book.spells[id];
        !s.rank.starts_with("Rank") || best.get(&s.name).map(|b| b.1) == Some(*id)
    });
    keep.push(ATTACK);
    if c.id == 3 {
        keep.push(AUTO_SHOT);
    }
    keep.sort_unstable();
    keep.dedup();
    keep
}

/// The main bar (buttons 0-11): the class's main spells by name, the highest it knows of each, then other spells
/// that do damage or heal, in the order known. Values: spell ids.
pub fn main_bar(book: &Book, c: &ClassDef, known: &[u32]) -> Vec<u32> {
    let by_name = |name: &str| -> Option<u32> {
        if name == "Attack" {
            return Some(ATTACK);
        }
        if name == "Auto Shot" && known.contains(&AUTO_SHOT) {
            return Some(AUTO_SHOT);
        }
        known.iter().filter_map(|id| book.spell(*id)).filter(|s| s.name == name && !s.passive()).max_by_key(|s| (s.spell_level, s.id)).map(|s| s.id)
    };
    let mut bar: Vec<u32> = c.bar.iter().filter_map(|n| by_name(n)).collect();
    bar.dedup();
    for id in known {
        if bar.len() >= 12 {
            break;
        }
        if bar.contains(id) {
            continue;
        }
        if let Some(s) = book.spell(*id) {
            let hits = s.effects.iter().any(|e| matches!(e.kind, 2 | 10 | 58 | 17 | 31 | 121));
            if hits && !s.passive() && !bar.iter().any(|b| book.spell(*b).is_some_and(|x| x.name == s.name)) {
                bar.push(*id);
            }
        }
    }
    bar.truncate(12);
    bar
}

/// The class chosen, kept in the Dens folder, with the player's own bar per class (button -> packed action).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pick {
    pub class: u8,
    pub level: u32,
    pub buttons: BTreeMap<String, BTreeMap<u8, u32>>,
}

impl Pick {
    pub fn class_def(&self) -> Option<&'static ClassDef> {
        by_id(self.class)
    }
    pub fn load(dir: &Path) -> Option<Pick> {
        let t = std::fs::read_to_string(dir.join(FILE)).ok()?;
        let v: Value = serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()?;
        let c = by_word(v.get("class")?.as_str()?)?;
        let level = v.get("level").and_then(Value::as_u64).unwrap_or(MAX_LEVEL as u64).clamp(1, MAX_LEVEL as u64) as u32;
        let mut buttons = BTreeMap::new();
        if let Some(m) = v.get("buttons").and_then(Value::as_object) {
            for (class, bs) in m {
                let Some(bs) = bs.as_object() else { continue };
                let mut one = BTreeMap::new();
                for (k, a) in bs {
                    if let (Ok(b), Some(a)) = (k.parse::<u8>(), a.as_u64()) {
                        if (b as usize) < BUTTONS {
                            one.insert(b, a as u32);
                        }
                    }
                }
                buttons.insert(class.clone(), one);
            }
        }
        Some(Pick { class: c.id, level, buttons })
    }
    pub fn save(&self, dir: &Path) -> bool {
        let Some(c) = self.class_def() else { return false };
        let buttons: serde_json::Map<String, Value> = self
            .buttons
            .iter()
            .map(|(k, bs)| (k.clone(), Value::Object(bs.iter().map(|(b, a)| (b.to_string(), json!(a))).collect())))
            .collect();
        let v = json!({"class": c.word, "level": self.level, "buttons": buttons});
        let _ = std::fs::create_dir_all(dir);
        crate::companion::write_json(dir, FILE, &v)
    }
    /// The player put something on a button (CMSG_SET_ACTION_BUTTON): kept for this class.
    pub fn set_button(&mut self, button: u8, packed: u32) {
        if (button as usize) < BUTTONS {
            if let Some(c) = self.class_def() {
                self.buttons.entry(c.word.to_string()).or_default().insert(button, packed);
            }
        }
    }
}

/// What the world sends for the chosen class: its numbers, its spells, its 144 buttons (packed).
#[derive(Debug, Clone, PartialEq)]
pub struct Kit {
    pub class: u8,
    pub stats: Stats,
    pub spells: Vec<u32>,
    pub buttons: Vec<u32>,
}

impl Kit {
    pub fn class_def(&self) -> &'static ClassDef {
        by_id(self.class).unwrap_or(&CLASSES[0])
    }
}

/// A button's value: an action with its type (0: a spell).
pub fn packed(action: u32, kind: u8) -> u32 {
    (action & 0x00FF_FFFF) | (kind as u32) << 24
}

pub fn kit(book: &Book, pick: &Pick) -> Option<Kit> {
    let c = pick.class_def()?;
    let spells = learn(book, c, pick.level);
    let mut buttons = vec![0u32; BUTTONS];
    for (i, id) in main_bar(book, c, &spells).into_iter().enumerate() {
        buttons[i] = packed(id, 0);
    }
    if let Some(own) = pick.buttons.get(c.word) {
        for (&b, &a) in own {
            buttons[b as usize] = a;
        }
    }
    Some(Kit { class: c.id, stats: stats(c, pick.level), spells, buttons })
}

/// SMSG_INITIAL_SPELLS: u8 0, u16 count, each u32 spell and u16 0, then u16 0 cooldowns.
pub fn initial_spells_body(spells: &[u32]) -> Vec<u8> {
    let n = spells.len().min(u16::MAX as usize);
    let mut b = Vec::with_capacity(5 + n * 6);
    b.push(0);
    b.extend_from_slice(&(n as u16).to_le_bytes());
    for s in &spells[..n] {
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
    }
    b.extend_from_slice(&0u16.to_le_bytes());
    b
}

/// SMSG_ACTION_BUTTONS: u8 state (0 at login, 1 a change of bars, as AzerothCore's talent spec switch), 144 u32.
pub fn action_buttons_body(buttons: &[u32], state: u8) -> Vec<u8> {
    let mut b = Vec::with_capacity(1 + BUTTONS * 4);
    b.push(state);
    for i in 0..BUTTONS {
        b.extend_from_slice(&buttons.get(i).copied().unwrap_or(0).to_le_bytes());
    }
    b
}

/// CMSG_SET_ACTION_BUTTON: u8 button, u32 packed action. None when short.
pub fn read_set_button(body: &[u8]) -> Option<(u8, u32)> {
    let b = *body.first()?;
    let a = body.get(1..5)?;
    Some((b, u32::from_le_bytes([a[0], a[1], a[2], a[3]])))
}

/// `.den class` alone: the classes, and what you are.
pub fn list_line(pick: Option<&Pick>) -> String {
    let words: Vec<&str> = CLASSES.iter().map(|c| c.word).collect();
    let now = match pick.and_then(|p| p.class_def().map(|c| (c, p.level))) {
        Some((c, l)) => format!("You are a level {l} {}. ", c.name),
        None => "You are a level 1 character with no class spells yet. ".into(),
    };
    format!("{now}.den class <class> [level 1-80]: {}.", words.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::fake;

    fn book() -> Book {
        fake::book()
    }

    #[test]
    fn every_class_has_a_word_lines_and_a_bar() {
        assert_eq!(CLASSES.len(), 10);
        for c in CLASSES {
            assert_eq!(by_word(c.word).map(|x| x.id), Some(c.id));
            assert!(!c.lines.is_empty() && c.bar.len() <= 12 && !c.bar.is_empty());
            assert!(!crate::companion::branded(c.name) && c.bar.iter().all(|n| !crate::companion::branded(n)));
        }
        assert_eq!(by_word("Death Knight").map(|c| c.id), Some(6));
        assert_eq!(by_word("dk").map(|c| c.id), Some(6));
        assert_eq!(by_word("mages").map(|c| c.id), Some(8));
        assert!(by_word("bard").is_none());
        assert_eq!(class_bit(8), 128);
        assert_eq!(class_bit(11), 1024);
        assert_eq!(level_arg(""), Ok(80));
        assert_eq!(level_arg("60"), Ok(60));
        assert!(level_arg("81").is_err() && level_arg("x").is_err());
    }

    #[test]
    fn numbers_by_class_and_level() {
        let mage = stats(by_word("mage").unwrap(), 80);
        assert_eq!((mage.power, mage.health, mage.max_power), (MANA, 16100, 11620));
        let war = stats(by_word("warrior").unwrap(), 80);
        assert_eq!((war.power, war.max_power, war.attack_ms), (RAGE, 1000, 2600));
        assert!(war.health > mage.health && war.attack_power > mage.attack_power && mage.spell_power > war.spell_power);
        assert_eq!(stats(by_word("rogue").unwrap(), 10).max_power, 100);
        assert_eq!(stats(by_word("deathknight").unwrap(), 80).power, RUNIC);
        assert_eq!(stats(by_word("mage").unwrap(), 200).level, 80);
    }

    #[test]
    fn a_mage_learns_the_highest_rank_its_level_allows() {
        let b = book();
        let mage = by_word("mage").unwrap();
        let at80 = learn(&b, mage, 80);
        assert!(at80.contains(&143) && !at80.contains(&133), "Fireball's next rank replaces the first: {at80:?}");
        assert!(at80.contains(&2136) && at80.contains(&1459) && at80.contains(&5143) && at80.contains(&ATTACK));
        assert!(!at80.contains(&78), "the warrior's strike is another class's");
        let at1 = learn(&b, mage, 1);
        assert!(at1.contains(&133) && !at1.contains(&143) && !at1.contains(&2136), "{at1:?}");
        let war = learn(&b, by_word("warrior").unwrap(), 80);
        assert_eq!(war, vec![78, ATTACK]);
        let hunter = learn(&b, by_word("hunter").unwrap(), 80);
        assert!(hunter.contains(&AUTO_SHOT));
    }

    #[test]
    fn the_bar_holds_the_main_spells_and_the_players_own_buttons() {
        let b = book();
        let mage = by_word("mage").unwrap();
        let known = learn(&b, mage, 80);
        let bar = main_bar(&b, mage, &known);
        assert_eq!(&bar[..4], &[143, 5143, 2136, 1459], "Fireball, Arcane Missiles, Fire Blast, Arcane Intellect in the bar's order");
        let mut pick = Pick { class: 8, level: 80, buttons: BTreeMap::new() };
        pick.set_button(5, packed(2136, 0));
        let k = kit(&b, &pick).unwrap();
        assert_eq!(k.buttons.len(), BUTTONS);
        assert_eq!(k.buttons[0], 143);
        assert_eq!(k.buttons[5], 2136);
        assert_eq!(k.stats.level, 80);
        assert_eq!(packed(6603, 0x40), 6603 | 0x4000_0000);
    }

    #[test]
    fn the_bodies_byte_for_byte() {
        let s = initial_spells_body(&[133, 6603]);
        assert_eq!(s, vec![0, 2, 0, 133, 0, 0, 0, 0, 0, 0xCB, 0x19, 0, 0, 0, 0, 0, 0]);
        let a = action_buttons_body(&[143, 0, 6603], 1);
        assert_eq!(a.len(), 1 + 144 * 4);
        assert_eq!(a[0], 1);
        assert_eq!(&a[1..5], &143u32.to_le_bytes());
        assert_eq!(&a[9..13], &6603u32.to_le_bytes());
        assert_eq!(read_set_button(&[3, 0x8F, 0, 0, 0]), Some((3, 143)));
        assert_eq!(read_set_button(&[3, 1]), None);
    }

    #[test]
    fn the_choice_is_kept_with_the_players_buttons() {
        let dir = std::env::temp_dir().join(format!("den-lite-pick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Pick::load(&dir).is_none());
        let mut p = Pick { class: 6, level: 70, buttons: BTreeMap::new() };
        p.set_button(2, 49909);
        p.set_button(200, 1);
        assert!(p.save(&dir));
        let back = Pick::load(&dir).unwrap();
        assert_eq!(back, p);
        assert_eq!(back.buttons["deathknight"].len(), 1);
        assert!(list_line(Some(&back)).starts_with("You are a level 70 Death Knight."));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
