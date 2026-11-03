//! Gear sets (owner 3 Nov: "gear sets selector"): a whole look at once, for the class and spec picked, at a raid
//! tier (T1-T10, the classes' own armour sets) or one of the player's own saved sets. The look only, as gear.rs:
//! every visible slot changes at once for everyone in the Den; the weapons are made real items too (sandbox.rs), so
//! spells that need a weapon (a rogue's Backstab, a hunter's shots) can be cast; the tier also stands for the
//! sandbox's numbers (classes::geared: more health, spell or attack power, critical chance, hit).
//!
//! Where the items come from: the player's own `ItemSet.dbc` (each set's name and items) and `Item.dbc` (each item's
//! slot, class and subclass), copied by the launcher into the Dens folder; never shipped. A tier's set is found by
//! its name (the role's version first: Raiment / Regalia / Garb for healers and casters, Battlegear / Harness for
//! the others, Armor / Plate for tanks); the slots a set has no piece for (the newer sets have five) and the weapons
//! are picked from the same era's items of the right kind, the same ones every time.
//!
//! Saved sets: `.den gearset save <name>` keeps what you wear in `<dens>/den-lite.gearsets.json`.

use crate::gear::{Items, SLOTS_N};
use crate::specs::{Role, SpecDef};
use crate::spells::Table;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const ITEM_SET: &str = "ItemSet.dbc";
pub const FILE: &str = "den-lite.gearsets.json";
pub const MAX_SAVED: usize = 20;

/// ItemSet.dbc (3.3.5a: 53 fields, 212 bytes): id, the name (16 locales and flags), 17 items from field 18.
#[derive(Debug, Default, Clone)]
pub struct Sets {
    pub sets: Vec<(u32, String, Vec<u32>)>,
}

impl Sets {
    pub fn from_bytes(b: &[u8]) -> Sets {
        let mut sets = Vec::new();
        if let Some(t) = Table::parse(b).filter(|t| t.fields == 53 && t.size == 212) {
            for k in 0..t.n {
                let r = t.row(k);
                let items: Vec<u32> = (18..35).map(|f| Table::u(r, f)).filter(|i| *i != 0).collect();
                sets.push((Table::u(r, 0), t.text(r, 1), items));
            }
        }
        Sets { sets }
    }
    pub fn read(dir: &Path) -> Sets {
        std::fs::read(dir.join(ITEM_SET)).map(|b| Sets::from_bytes(&b)).unwrap_or_default()
    }
    /// The set by one of these names (in order; the lowest id of a name: its first version), else by the first
    /// name's first word with the role's word.
    pub fn find(&self, names: &[&str], role: Role) -> Option<&(u32, String, Vec<u32>)> {
        for n in names {
            if let Some(s) = self.sets.iter().filter(|s| s.1.eq_ignore_ascii_case(n) && !s.2.is_empty()).min_by_key(|s| s.0) {
                return Some(s);
            }
        }
        let stem = names.first()?.split_whitespace().next()?.trim_end_matches("'s").to_lowercase();
        if stem.len() < 4 || stem == "the" || stem == "vestments" || stem == "battlegear" || stem == "regalia" {
            return None;
        }
        let pref = role_words(role);
        self.sets
            .iter()
            .filter(|s| s.1.to_lowercase().starts_with(&stem) && !s.2.is_empty())
            .min_by_key(|s| (pref.iter().position(|w| s.1.to_lowercase().contains(w)).unwrap_or(9), s.0))
    }
}

/// The words a role's version of a set has in its name, best first.
fn role_words(role: Role) -> &'static [&'static str] {
    match role {
        Role::Healer => &["raiment", "garb", "vestments", "regalia"],
        Role::Caster => &["regalia", "garb", "raiment"],
        Role::Tank => &["plate", "armor"],
        Role::Melee | Role::Ranged => &["battlegear", "harness", "armor", "pursuit"],
    }
}

/// A class's raid tiers: (tier, names: the role versions; healer/caster, then melee, then tank where they differ).
pub fn tiers(class: u8) -> &'static [(u8, &'static [&'static str])] {
    match class {
        1 => &[(1, &["Battlegear of Might"]), (2, &["Battlegear of Wrath"]), (3, &["Dreadnaught's Battlegear"]), (4, &["Warbringer Battlegear", "Warbringer Armor"]),
            (5, &["Destroyer Battlegear", "Destroyer Armor"]), (6, &["Onslaught Battlegear", "Onslaught Armor"]), (7, &["Dreadnaught Battlegear", "Dreadnaught Plate"]),
            (8, &["Siegebreaker Battlegear", "Siegebreaker Plate"]), (9, &["Wrynn's Battlegear", "Wrynn's Plate", "Hellscream's Battlegear"]),
            (10, &["Ymirjar Lord's Battlegear", "Ymirjar Lord's Plate"])],
        2 => &[(1, &["Lawbringer Armor"]), (2, &["Judgement Armor"]), (3, &["Redemption Armor"]), (4, &["Justicar Raiment", "Justicar Battlegear", "Justicar Armor"]),
            (5, &["Crystalforge Raiment", "Crystalforge Battlegear", "Crystalforge Armor"]), (6, &["Lightbringer Raiment", "Lightbringer Battlegear", "Lightbringer Armor"]),
            (7, &["Redemption Regalia", "Redemption Battlegear", "Redemption Plate"]), (8, &["Aegis Regalia", "Aegis Battlegear", "Aegis Plate"]),
            (9, &["Turalyon's Garb", "Turalyon's Battlegear", "Turalyon's Plate"]), (10, &["Lightsworn Garb", "Lightsworn Battlegear", "Lightsworn Plate"])],
        3 => &[(1, &["Giantstalker Armor"]), (2, &["Dragonstalker Armor"]), (3, &["Cryptstalker Armor"]), (4, &["Demon Stalker Armor"]), (5, &["Rift Stalker Armor"]),
            (6, &["Gronnstalker's Armor"]), (7, &["Cryptstalker Battlegear"]), (8, &["Scourgestalker Battlegear"]), (9, &["Windrunner's Battlegear", "Windrunner's Pursuit"]),
            (10, &["Ahn'Kahar Blood Hunter's Battlegear"])],
        4 => &[(1, &["Nightslayer Armor"]), (2, &["Bloodfang Armor"]), (3, &["Bonescythe Armor"]), (4, &["Netherblade"]), (5, &["Deathmantle"]), (6, &["Slayer's Armor"]),
            (7, &["Bonescythe Battlegear"]), (8, &["Terrorblade Battlegear"]), (9, &["VanCleef's Battlegear", "Garona's Battlegear"]), (10, &["Shadowblade's Battlegear"])],
        5 => &[(1, &["Vestments of Prophecy"]), (2, &["Vestments of Transcendence"]), (3, &["Vestments of Faith"]), (4, &["Incarnate Raiment", "Incarnate Regalia"]),
            (5, &["Avatar Raiment", "Avatar Regalia"]), (6, &["Vestments of Absolution", "Absolution Regalia"]), (7, &["Regalia of Faith", "Garb of Faith"]),
            (8, &["Sanctification Regalia", "Sanctification Garb"]), (9, &["Velen's Regalia", "Velen's Raiment"]), (10, &["Crimson Acolyte's Regalia", "Crimson Acolyte's Raiment"])],
        6 => &[(7, &["Scourgeborne Battlegear", "Scourgeborne Plate"]), (8, &["Darkruned Battlegear", "Darkruned Plate"]), (9, &["Thassarian's Battlegear", "Thassarian's Plate", "Koltira's Battlegear"]),
            (10, &["Scourgelord's Battlegear", "Scourgelord's Plate"])],
        7 => &[(1, &["The Earthfury"]), (2, &["The Ten Storms"]), (3, &["The Earthshatterer"]), (4, &["Cyclone Raiment", "Cyclone Regalia", "Cyclone Harness"]),
            (5, &["Cataclysm Raiment", "Cataclysm Regalia", "Cataclysm Harness"]), (6, &["Skyshatter Raiment", "Skyshatter Regalia", "Skyshatter Harness"]),
            (7, &["Earthshatter Garb", "Earthshatter Regalia", "Earthshatter Battlegear"]), (8, &["Worldbreaker Garb", "Worldbreaker Regalia", "Worldbreaker Battlegear"]),
            (9, &["Nobundo's Garb", "Nobundo's Regalia", "Nobundo's Battlegear"]), (10, &["Frost Witch's Garb", "Frost Witch's Regalia", "Frost Witch's Battlegear"])],
        8 => &[(1, &["Arcanist Regalia"]), (2, &["Netherwind Regalia"]), (3, &["Frostfire Regalia"]), (4, &["Aldor Regalia"]), (5, &["Tirisfal Regalia"]), (6, &["Tempest Regalia"]),
            (7, &["Frostfire Garb"]), (8, &["Kirin Tor Garb"]), (9, &["Khadgar's Regalia", "Sunstrider's Regalia"]), (10, &["Bloodmage's Regalia"])],
        9 => &[(1, &["Felheart Raiment"]), (2, &["Nemesis Raiment"]), (3, &["Plagueheart Raiment"]), (4, &["Voidheart Raiment"]), (5, &["Corruptor Raiment"]), (6, &["Malefic Raiment"]),
            (7, &["Plagueheart Garb"]), (8, &["Deathbringer Garb"]), (9, &["Kel'Thuzad's Regalia", "Gul'dan's Regalia"]), (10, &["Dark Coven's Regalia", "Dark Coven's Garb"])],
        11 => &[(1, &["Cenarion Raiment"]), (2, &["Stormrage Raiment"]), (3, &["Dreamwalker Raiment"]), (4, &["Malorne Raiment", "Malorne Regalia", "Malorne Harness"]),
            (5, &["Nordrassil Raiment", "Nordrassil Regalia", "Nordrassil Harness"]), (6, &["Thunderheart Raiment", "Thunderheart Regalia", "Thunderheart Harness"]),
            (7, &["Dreamwalker Garb", "Dreamwalker Regalia", "Dreamwalker Battlegear"]), (8, &["Nightsong Garb", "Nightsong Regalia", "Nightsong Battlegear"]),
            (9, &["Malfurion's Garb", "Malfurion's Regalia", "Malfurion's Battlegear"]), (10, &["Lasherweave Garb", "Lasherweave Regalia", "Lasherweave Battlegear"])],
        _ => &[],
    }
}

/// The names in the order a role wants them (its own version first).
fn for_role(names: &[&'static str], role: Role) -> Vec<&'static str> {
    let pref = role_words(role);
    let mut v: Vec<&'static str> = names.to_vec();
    v.sort_by_key(|n| pref.iter().position(|w| n.to_lowercase().contains(w)).unwrap_or(9));
    v
}

/// The armour a class wears (ItemSubclassArmor): cloth 1, leather 2, mail 3, plate 4.
pub fn armour_of(class: u8) -> u32 {
    match class {
        1 | 2 | 6 => 4,
        3 | 7 => 3,
        4 | 11 => 2,
        _ => 1,
    }
}

/// The entries of an era (by the tier): the items' numbers grew with the game's expansions.
fn era(tier: u8) -> (u32, u32) {
    match tier {
        0..=3 => (16_000, 24_999),
        4..=6 => (27_000, 35_999),
        _ => (39_000, 52_999),
    }
}

/// A spec's weapons: (slot, the (item class, subclass) kinds it takes, the inventory types it takes).
pub fn weapons(spec: &SpecDef) -> Vec<(u8, &'static [(u32, u32)], &'static [u32])> {
    const TWO_H: &[(u32, u32)] = &[(2, 8), (2, 1), (2, 5)];
    const ONE_H: &[(u32, u32)] = &[(2, 7), (2, 0), (2, 4)];
    const MACE_1H: &[(u32, u32)] = &[(2, 4)];
    const DAGGER: &[(u32, u32)] = &[(2, 15)];
    const STAFF: &[(u32, u32)] = &[(2, 10)];
    const SHIELD: &[(u32, u32)] = &[(4, 6)];
    const BOW: &[(u32, u32)] = &[(2, 2), (2, 3), (2, 18)];
    const POLEARM: &[(u32, u32)] = &[(2, 6), (2, 1)];
    const WAND: &[(u32, u32)] = &[(2, 19)];
    const FIST_AXE: &[(u32, u32)] = &[(2, 0), (2, 13), (2, 4)];
    const MH: &[u32] = &[13, 21];
    const OH: &[u32] = &[13, 22];
    const H2: &[u32] = &[17];
    const SH: &[u32] = &[14];
    const RANGED: &[u32] = &[15, 26];
    const WANDS: &[u32] = &[26];
    match (spec.class, spec.word) {
        (1, "protection") | (2, "protection") => vec![(15, ONE_H, MH), (16, SHIELD, SH)],
        (1, "fury") => vec![(15, TWO_H, H2), (16, TWO_H, H2)],
        (1, _) | (2, "retribution") | (6, "blood") | (6, "unholy") => vec![(15, TWO_H, H2)],
        (2, _) | (7, "elemental") | (7, "restoration") => vec![(15, MACE_1H, MH), (16, SHIELD, SH)],
        (3, _) => vec![(15, POLEARM, H2), (17, BOW, RANGED)],
        (4, "combat") => vec![(15, ONE_H, MH), (16, ONE_H, OH)],
        (4, _) => vec![(15, DAGGER, MH), (16, DAGGER, OH)],
        (6, _) => vec![(15, ONE_H, MH), (16, ONE_H, OH)],
        (7, _) => vec![(15, FIST_AXE, MH), (16, FIST_AXE, OH)],
        (11, "feral") => vec![(15, STAFF, H2)],
        (5, _) | (8, _) | (9, _) | (11, _) => vec![(15, STAFF, H2), (17, WAND, WANDS)],
        _ => vec![],
    }
}

/// The same pick every time: of the era's items of these kinds and inventory types (the middle one; with none in
/// the era, of all).
pub fn pick(items: &Items, tier: u8, kinds: &[(u32, u32)], invs: &[u32]) -> Option<u32> {
    let mut all: Vec<u32> = items
        .by_id
        .iter()
        .filter(|(id, &(display, inv))| display != 0 && invs.contains(&inv) && items.kinds.get(id).is_some_and(|k| kinds.contains(k)))
        .map(|(id, _)| *id)
        .collect();
    all.sort_unstable();
    let (lo, hi) = era(tier);
    let in_era: Vec<u32> = all.iter().copied().filter(|id| (lo..=hi).contains(id)).collect();
    let from = if in_era.is_empty() { &all } else { &in_era };
    from.get(from.len() / 2).copied()
}

/// The armour slots a set may leave empty, with their inventory types (head, shoulders, chest, waist, legs, feet,
/// wrists, hands; the back: a cloak, cloth).
const ARMOUR_SLOTS: &[(u8, &[u32])] = &[(0, &[1]), (2, &[3]), (4, &[5, 20]), (5, &[6]), (6, &[7]), (7, &[8]), (8, &[9]), (9, &[10])];

/// A tier's whole look for a spec: (the set's name, slot -> item). None: no such tier for the class, or the
/// player's tables don't have it.
pub fn tier_look(sets: &Sets, items: &Items, spec: &SpecDef, tier: u8) -> Option<(String, BTreeMap<u8, u32>)> {
    let names = tiers(spec.class).iter().find(|t| t.0 == tier)?.1;
    let set = sets.find(&for_role(names, spec.role), spec.role)?;
    let mut look: BTreeMap<u8, u32> = BTreeMap::new();
    for id in &set.2 {
        if let Some(s) = items.slot(*id) {
            look.entry(s).or_insert(*id);
        }
    }
    if look.is_empty() {
        return None;
    }
    let armour = set.2.iter().filter_map(|id| items.kinds.get(id)).find(|k| k.0 == 4 && (1..=4).contains(&k.1)).map(|k| k.1).unwrap_or(armour_of(spec.class));
    for &(slot, invs) in ARMOUR_SLOTS {
        if !look.contains_key(&slot) {
            if let Some(id) = pick(items, tier, &[(4, armour)], invs) {
                look.insert(slot, id);
            }
        }
    }
    if let Some(id) = pick(items, tier, &[(4, 1)], &[16]) {
        look.entry(14).or_insert(id);
    }
    for (slot, kinds, invs) in weapons(spec) {
        if let Some(id) = pick(items, tier, kinds, invs) {
            look.insert(slot, id);
        }
    }
    Some((set.1.clone(), look))
}

/// The tiers a class's player can wear here (the player's tables have them).
pub fn available(sets: &Sets, items: &Items, spec: &SpecDef) -> Vec<u8> {
    tiers(spec.class).iter().map(|t| t.0).filter(|t| tier_look(sets, items, spec, *t).is_some()).collect()
}

/// "t7", "T7", "tier 7", "7": a tier.
pub fn tier_word(words: &[&str]) -> Option<u8> {
    let w = match words {
        ["tier", n] => n.to_string(),
        [w] => w.to_lowercase().trim_start_matches("tier").trim_start_matches('t').to_string(),
        _ => return None,
    };
    w.parse::<u8>().ok().filter(|t| (1..=10).contains(t))
}

/// The player's own sets (`<dens>/den-lite.gearsets.json`: name -> slot word -> item).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Saved {
    pub sets: BTreeMap<String, (BTreeMap<u8, u32>, u8)>,
}

impl Saved {
    pub fn load(dir: &Path) -> Saved {
        let mut out = Saved::default();
        let Some(v) = std::fs::read_to_string(dir.join(FILE)).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok()) else { return out };
        if let Some(m) = v.get("sets").and_then(Value::as_object) {
            for (name, s) in m.iter().take(MAX_SAVED) {
                let Some(n) = crate::classes::loadout_name(name) else { continue };
                let mut slots = BTreeMap::new();
                if let Some(o) = s.get("slots").and_then(Value::as_object) {
                    for (k, e) in o {
                        if let (Some(sl), Some(e)) = (crate::gear::slot_word(k), e.as_u64()) {
                            slots.insert(sl, e as u32);
                        }
                    }
                }
                let tier = s.get("tier").and_then(Value::as_u64).unwrap_or(0).min(10) as u8;
                out.sets.insert(n, (slots, tier));
            }
        }
        out
    }
    pub fn save(&self, dir: &Path) -> bool {
        let sets: serde_json::Map<String, Value> = self
            .sets
            .iter()
            .map(|(n, (slots, tier))| {
                let o: serde_json::Map<String, Value> = slots.iter().map(|(s, e)| (crate::gear::slot_name(*s).to_string(), json!(*e))).collect();
                (n.clone(), json!({"slots": Value::Object(o), "tier": *tier}))
            })
            .collect();
        let _ = std::fs::create_dir_all(dir);
        crate::companion::write_json(dir, FILE, &json!({"sets": Value::Object(sets)}))
    }
    /// What is worn now, by a name (a tier word is a tier, not a name). Err: why not.
    pub fn put(&mut self, name: &str, slots: &BTreeMap<u8, u32>, tier: u8) -> Result<String, String> {
        let n = crate::classes::loadout_name(name).ok_or("Name it: .den gearset save <name>")?;
        if tier_word(&[n.as_str()]).is_some() || n == "off" || n == "list" {
            return Err(format!("'{n}' is a word the gear sets use: pick another name."));
        }
        if !self.sets.contains_key(&n) && self.sets.len() >= MAX_SAVED {
            return Err(format!("{MAX_SAVED} saved sets at most: .den gearset delete <name> first."));
        }
        let worn: BTreeMap<u8, u32> = slots.iter().filter(|(s, _)| **s < SLOTS_N).map(|(s, e)| (*s, *e)).collect();
        self.sets.insert(n.clone(), (worn, tier));
        Ok(n)
    }
}

/// Test helpers: the tables as the game writes them, from a few made-up rows (nothing of the game's own).
pub mod fake {
    use crate::spells::fake::table;

    /// ItemSet.dbc from (id, name, items): the name in the first locale.
    pub fn sets(rows: &[(u32, &str, &[u32])]) -> Vec<u8> {
        let mut strings = vec![0u8];
        let mut v = Vec::new();
        for (id, name, items) in rows {
            let mut r = vec![0u32; 53];
            r[0] = *id;
            r[1] = strings.len() as u32;
            strings.extend_from_slice(name.as_bytes());
            strings.push(0);
            for (k, i) in items.iter().take(17).enumerate() {
                r[18 + k] = *i;
            }
            v.push(r);
        }
        table(53, &v, &strings)
    }

    /// Item.dbc rows (id, class, subclass, display, inventory type).
    pub fn items(rows: &[(u32, u32, u32, u32, u32)]) -> Vec<u8> {
        let v: Vec<Vec<u32>> = rows.iter().map(|&(id, c, sc, d, inv)| vec![id, c, sc, 0, 1, d, inv, 0]).collect();
        table(8, &v, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specs::by_word;

    fn world() -> (Sets, Items) {
        // a mage's T1 (8 cloth pieces) and T10 (5 pieces: head, shoulders, chest, hands, legs), a warrior's T10 in two
        // versions, other cloth to fill the slots, staves, wands, swords and a shield
        let sets = Sets::from_bytes(&fake::sets(&[
            (201, "Arcanist Regalia", &[16795, 16797, 16798, 16801, 16796, 16800, 16799, 16802]),
            (803, "Bloodmage's Regalia", &[51281, 51284, 51283, 51280, 51282]),
            (900, "Ymirjar Lord's Battlegear", &[51227, 51229]),
            (901, "Ymirjar Lord's Plate", &[51221, 51224]),
        ]));
        let items = Items::from_bytes(&fake::items(&[
            (16795, 4, 1, 100, 1), (16797, 4, 1, 101, 3), (16798, 4, 1, 102, 20), (16801, 4, 1, 103, 10),
            (16796, 4, 1, 104, 7), (16800, 4, 1, 105, 8), (16799, 4, 1, 106, 9), (16802, 4, 1, 107, 6),
            (51281, 4, 1, 200, 1), (51284, 4, 1, 201, 3), (51283, 4, 1, 202, 20), (51280, 4, 1, 203, 10), (51282, 4, 1, 204, 7),
            (50000, 4, 1, 300, 6), (50001, 4, 1, 301, 8), (50002, 4, 1, 302, 9), (50100, 4, 1, 303, 16),
            (51227, 4, 4, 400, 1), (51229, 4, 4, 401, 5), (51221, 4, 4, 402, 1), (51224, 4, 4, 403, 5),
            (50731, 2, 10, 500, 17), (18000, 2, 10, 501, 17), (50033, 2, 19, 502, 26),
            (50300, 2, 7, 600, 13), (50400, 4, 6, 601, 14), (50500, 2, 8, 602, 17),
        ]));
        (sets, items)
    }

    #[test]
    fn a_tier_is_the_classs_set_with_the_rest_filled_and_its_weapons() {
        let (sets, items) = world();
        let fire = by_word(8, "fire").unwrap();
        let (name, look) = tier_look(&sets, &items, fire, 10).unwrap();
        assert_eq!(name, "Bloodmage's Regalia");
        assert_eq!((look[&0], look[&2], look[&4], look[&9], look[&6]), (51281, 51284, 51283, 51280, 51282));
        assert_eq!((look[&5], look[&7], look[&8]), (50000, 50001, 50002), "the slots it has no piece for: the era's cloth");
        assert_eq!(look[&14], 50100, "a cloak");
        assert_eq!((look[&15], look[&17]), (50731, 50033), "a staff of its era and a wand");
        let (t1, l1) = tier_look(&sets, &items, fire, 1).unwrap();
        assert_eq!((t1.as_str(), l1[&0], l1[&4]), ("Arcanist Regalia", 16795, 16798));
        assert_eq!(l1[&15], 18000, "a staff of the old era");
        assert!(tier_look(&sets, &items, fire, 5).is_none(), "not in the player's tables");
        assert_eq!(available(&sets, &items, fire), vec![1, 10]);
        // a tank wants the plate version, a fighter the battlegear
        let prot = by_word(1, "prot").unwrap();
        let arms = by_word(1, "arms").unwrap();
        assert_eq!(tier_look(&sets, &items, prot, 10).unwrap().0, "Ymirjar Lord's Plate");
        let (an, al) = tier_look(&sets, &items, arms, 10).unwrap();
        assert_eq!((an.as_str(), al[&15]), ("Ymirjar Lord's Battlegear", 50500));
        let (_, pl) = tier_look(&sets, &items, prot, 10).unwrap();
        assert_eq!((pl[&15], pl[&16]), (50300, 50400), "a sword and a shield");
        for c in [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 11] {
            for (_, names) in tiers(c) {
                assert!(names.iter().all(|n| !crate::companion::branded(n)), "{names:?}");
            }
        }
    }

    #[test]
    fn tier_words_and_saved_sets() {
        assert_eq!(tier_word(&["t7"]), Some(7));
        assert_eq!(tier_word(&["T10"]), Some(10));
        assert_eq!(tier_word(&["tier", "3"]), Some(3));
        assert_eq!(tier_word(&["tier3"]), Some(3));
        assert_eq!(tier_word(&["t11"]), None);
        assert_eq!(tier_word(&["mine"]), None);
        let mut s = Saved::default();
        let worn: BTreeMap<u8, u32> = [(0u8, 51281u32), (15, 50731)].into_iter().collect();
        assert_eq!(s.put("My Raid", &worn, 10), Ok("my raid".to_string()));
        assert!(s.put("t7", &worn, 0).is_err() && s.put("!!", &worn, 0).is_err());
        let dir = std::env::temp_dir().join(format!("den-lite-gearsets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(s.save(&dir));
        assert_eq!(Saved::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
        let other = Sets::from_bytes(&crate::spells::fake::table(52, &[vec![1]], &[]));
        assert!(other.sets.is_empty(), "a table of another shape");
    }
}
