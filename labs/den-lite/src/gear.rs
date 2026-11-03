//! Any gear's look on the character (owner 3 Nov 10:40: "allow all mounts, all gear"): `.den gear <item>` puts an
//! item's appearance in its slot. Only the look: the character's visible item fields (PLAYER_VISIBLE_ITEM_n_ENTRYID,
//! field 283 + 2 n, update fields of 3.3.5a), which the game draws from its own Item.dbc by entry, as the ready-made
//! outfit already is. No item objects, no stats.
//!
//! Which slot an item goes in comes from the player's own `Item.dbc` (8 fields: id, class, subclass, sound, material,
//! display, inventory type, sheath), copied into the Dens folder by the launcher (`host/lite.rs` `game_tables`); with
//! it missing, `.den gear <slot> <item>` still works. The choice is kept in `<dens>/den-lite.gear.json` (slot -> item).
//! The table has no names (they are the server's): `.den gear list <slot>` lists entries; the game shows their icons.

use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FILE: &str = "den-lite.gear.json";
pub const ITEM_TABLE: &str = "Item.dbc";
/// PLAYER_VISIBLE_ITEM_1_ENTRYID.
pub const VISIBLE_ITEM: u16 = 283;
pub const SLOTS_N: u8 = 19;
/// How many entries `.den gear list` shows at a time.
pub const PAGE: usize = 30;

/// The equipment slots by name (EquipmentSlots).
pub const SLOTS: &[(&str, u8)] = &[
    ("head", 0), ("neck", 1), ("shoulders", 2), ("shoulder", 2), ("shirt", 3), ("chest", 4), ("robe", 4), ("waist", 5), ("belt", 5),
    ("legs", 6), ("pants", 6), ("feet", 7), ("boots", 7), ("wrists", 8), ("bracers", 8), ("hands", 9), ("gloves", 9), ("ring", 10),
    ("ring2", 11), ("trinket", 12), ("trinket2", 13), ("back", 14), ("cloak", 14), ("mainhand", 15), ("weapon", 15), ("offhand", 16),
    ("shield", 16), ("ranged", 17), ("tabard", 18),
];

/// Ready-made looks: (word, the items). TODO(owner): which looks to offer (these are well-known weapons; armour sets
/// to add once picked).
pub const PRESETS: &[(&str, &[u32])] = &[
    ("thunderfury", &[19019]),
    ("shadowmourne", &[49623]),
    ("warglaives", &[32837, 32838]),
    ("sulfuras", &[17182]),
    ("valanyr", &[46017]),
];

/// The ready-made outfit (world.rs OUTFIT): shirt, trousers, boots.
pub const OUTFIT: &[(u8, u32)] = &[(3, 4334), (6, 39), (7, 40)];

pub fn slot_word(w: &str) -> Option<u8> {
    SLOTS.iter().find(|(n, _)| *n == w).map(|s| s.1)
}

pub fn slot_name(slot: u8) -> &'static str {
    SLOTS.iter().find(|s| s.1 == slot).map(|s| s.0).unwrap_or("?")
}

/// The slot an inventory type goes in (InventoryType -> EquipmentSlots).
pub fn slot_of_inv(inv: u32) -> Option<u8> {
    Some(match inv {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        5 | 20 => 4,
        6 => 5,
        7 => 6,
        8 => 7,
        9 => 8,
        10 => 9,
        11 => 10,
        12 => 12,
        16 => 14,
        13 | 17 | 21 => 15,
        14 | 22 | 23 => 16,
        15 | 25 | 26 | 28 => 17,
        19 => 18,
        _ => return None,
    })
}

/// Item.dbc: entry -> (display, inventory type), and its (class, subclass) (2 a weapon, 4 armour: gearsets.rs).
#[derive(Debug, Default)]
pub struct Items {
    pub by_id: HashMap<u32, (u32, u32)>,
    pub kinds: HashMap<u32, (u32, u32)>,
}

impl Items {
    pub fn from_bytes(b: &[u8]) -> Items {
        let mut by_id = HashMap::new();
        let mut kinds = HashMap::new();
        if let Some(t) = crate::spells::Table::parse(b).filter(|t| t.fields == 8 && t.size == 32) {
            for k in 0..t.n {
                let r = t.row(k);
                let id = crate::spells::Table::u(r, 0);
                by_id.insert(id, (crate::spells::Table::u(r, 5), crate::spells::Table::u(r, 6)));
                kinds.insert(id, (crate::spells::Table::u(r, 1), crate::spells::Table::u(r, 2)));
            }
        }
        Items { by_id, kinds }
    }
    pub fn slot(&self, entry: u32) -> Option<u8> {
        self.by_id.get(&entry).and_then(|&(_, inv)| slot_of_inv(inv))
    }
    /// The entries for a slot (sorted), a page of them.
    pub fn list(&self, slot: u8, page: usize) -> (Vec<u32>, usize) {
        let mut v: Vec<u32> = self.by_id.iter().filter(|(_, &(d, inv))| d != 0 && slot_of_inv(inv) == Some(slot)).map(|(&id, _)| id).collect();
        v.sort_unstable();
        let n = v.len();
        (v.into_iter().skip(page * PAGE).take(PAGE).collect(), n)
    }
}

static ITEMS: Mutex<Option<(PathBuf, Arc<Items>)>> = Mutex::new(None);

pub fn items(dir: &Path) -> Arc<Items> {
    let mut g = ITEMS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((d, i)) = g.as_ref() {
        if d == dir && !i.by_id.is_empty() {
            return i.clone();
        }
    }
    let i = Arc::new(std::fs::read(dir.join(ITEM_TABLE)).map(|b| Items::from_bytes(&b)).unwrap_or_default());
    *g = Some((dir.to_path_buf(), i.clone()));
    i
}

/// What the character wears (slot -> item; 0 empty), and the gear set put on last (gearsets.rs: its name and the
/// raid tier it stands for in the sandbox's numbers; 0 none).
#[derive(Debug, Clone, PartialEq)]
pub struct Gear {
    pub slots: BTreeMap<u8, u32>,
    pub set_name: String,
    pub tier: u8,
}

impl Default for Gear {
    fn default() -> Gear {
        Gear { slots: OUTFIT.iter().copied().collect(), set_name: String::new(), tier: 0 }
    }
}

impl Gear {
    pub fn load(dir: &Path) -> Option<Gear> {
        let v: Value = serde_json::from_str(std::fs::read_to_string(dir.join(FILE)).ok()?.trim_start_matches('\u{feff}')).ok()?;
        let mut slots = BTreeMap::new();
        for (k, e) in v.get("slots")?.as_object()? {
            if let (Some(s), Some(e)) = (slot_word(k), e.as_u64()) {
                slots.insert(s, e as u32);
            }
        }
        let set_name = v.get("set").and_then(Value::as_str).unwrap_or("").chars().take(24).collect();
        let tier = v.get("tier").and_then(Value::as_u64).unwrap_or(0).min(10) as u8;
        Some(Gear { slots, set_name, tier })
    }
    pub fn save(&self, dir: &Path) -> bool {
        let m: serde_json::Map<String, Value> = self.slots.iter().map(|(s, e)| (slot_name(*s).to_string(), json!(e))).collect();
        let _ = std::fs::create_dir_all(dir);
        crate::companion::write_json(dir, FILE, &json!({"slots": Value::Object(m), "set": self.set_name.as_str(), "tier": self.tier}))
    }
    pub fn set(&mut self, slot: u8, entry: u32) {
        if slot < SLOTS_N {
            self.slots.insert(slot, entry);
        }
    }
    /// The visible item fields (every slot: what isn't worn is 0).
    pub fn fields(&self) -> Vec<(u16, u32)> {
        (0..SLOTS_N).map(|s| (VISIBLE_ITEM + 2 * s as u16, self.slots.get(&s).copied().unwrap_or(0))).collect()
    }
}

/// `.den gear <words>`: what to change. Err: a line to say.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    Show,
    Reset,
    Put(Vec<(u8, u32)>),
    List(u8, usize),
}

pub fn parse(words: &[&str], items: &Items) -> Result<Ask, String> {
    match words {
        [] | ["help"] => Ok(Ask::Show),
        ["reset"] | ["outfit"] => Ok(Ask::Reset),
        ["list", slot, rest @ ..] => {
            let s = slot_word(slot).ok_or_else(|| format!("No slot called '{slot}': head, shoulders, chest, legs, feet, hands, back, mainhand, offhand, ranged, tabard..."))?;
            Ok(Ask::List(s, rest.first().and_then(|p| p.parse::<usize>().ok()).unwrap_or(1).max(1) - 1))
        }
        [slot, item] if slot_word(slot).is_some() => {
            let s = slot_word(slot).unwrap_or(0);
            match *item {
                "none" | "off" | "0" => Ok(Ask::Put(vec![(s, 0)])),
                n => n.parse::<u32>().map(|e| Ask::Put(vec![(s, e)])).map_err(|_| "Usage: .den gear <slot> <item number>".into()),
            }
        }
        [word] if PRESETS.iter().any(|p| p.0 == *word) => {
            let ids = PRESETS.iter().find(|p| p.0 == *word).map(|p| p.1).unwrap_or(&[]);
            put_ids(ids, items)
        }
        ids => {
            let nums: Result<Vec<u32>, _> = ids.iter().map(|w| w.parse::<u32>()).collect();
            match nums {
                Ok(n) if !n.is_empty() => put_ids(&n, items),
                _ => Err(format!("Usage: .den gear <item number>..., .den gear <slot> <item>, .den gear list <slot>, .den gear reset; looks: {}.", PRESETS.iter().map(|p| p.0).collect::<Vec<_>>().join(", "))),
            }
        }
    }
}

/// Items into their slots (a second one-hand weapon goes in the off hand).
fn put_ids(ids: &[u32], items: &Items) -> Result<Ask, String> {
    if items.by_id.is_empty() {
        return Err("Your game's item table isn't in the Dens folder yet (the launcher copies it when Your Den starts): .den gear <slot> <item> works meanwhile.".into());
    }
    let mut out: Vec<(u8, u32)> = Vec::new();
    for &id in ids {
        let Some(mut s) = items.slot(id) else { return Err(format!("Item {id} isn't something to wear here.")) };
        if s == 15 && out.iter().any(|x| x.0 == 15) && items.by_id.get(&id).is_some_and(|x| matches!(x.1, 13 | 21)) {
            s = 16;
        }
        out.push((s, id));
    }
    Ok(Ask::Put(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::fake;

    fn items() -> Items {
        // id, class, subclass, sound, material, display, inventory type, sheath
        let rows = vec![
            vec![19019, 2, 7, 0, 1, 30606, 13, 3],
            vec![32837, 2, 7, 0, 1, 45479, 21, 3],
            vec![32838, 2, 7, 0, 1, 45481, 22, 3],
            vec![16955, 4, 4, 0, 1, 31000, 1, 0],
            vec![999, 15, 0, 0, 0, 0, 0, 0],
        ];
        Items::from_bytes(&fake::table(8, &rows, &[]))
    }

    #[test]
    fn items_go_in_their_slots() {
        let i = items();
        assert_eq!(i.slot(19019), Some(15));
        assert_eq!(i.slot(16955), Some(0));
        assert_eq!(i.slot(999), None);
        assert_eq!(parse(&["19019"], &i), Ok(Ask::Put(vec![(15, 19019)])));
        assert_eq!(parse(&["warglaives"], &i), Ok(Ask::Put(vec![(15, 32837), (16, 32838)])));
        assert_eq!(parse(&["19019", "19019"], &i), Ok(Ask::Put(vec![(15, 19019), (16, 19019)])), "a second one-hander: the off hand");
        assert_eq!(parse(&["head", "16955"], &i), Ok(Ask::Put(vec![(0, 16955)])));
        assert_eq!(parse(&["head", "none"], &i), Ok(Ask::Put(vec![(0, 0)])));
        assert_eq!(parse(&["reset"], &i), Ok(Ask::Reset));
        assert_eq!(parse(&["list", "mainhand", "2"], &i), Ok(Ask::List(15, 1)));
        assert!(parse(&["999"], &i).is_err() && parse(&["list", "nose"], &i).is_err() && parse(&["sparkly"], &i).is_err());
        assert!(parse(&["19019"], &Items::default()).unwrap_err().contains("item table"));
        assert_eq!(parse(&["head", "16955"], &Items::default()), Ok(Ask::Put(vec![(0, 16955)])), "a slot and an item need no table");
        let (page, n) = i.list(15, 0);
        assert_eq!((page, n), (vec![19019, 32837], 2));
        for (w, _) in PRESETS {
            assert!(!crate::companion::branded(w));
        }
    }

    #[test]
    fn the_fields_cover_every_slot_and_the_choice_is_kept() {
        let mut g = Gear::default();
        let f = g.fields();
        assert_eq!(f.len(), 19);
        assert_eq!(f[3], (283 + 6, 4334), "the shirt");
        assert_eq!(f[15], (283 + 30, 0));
        g.set(15, 19019);
        g.set(40, 1);
        (g.set_name, g.tier) = ("t10".into(), 10);
        assert_eq!(g.fields()[15].1, 19019);
        let dir = std::env::temp_dir().join(format!("den-lite-gear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(g.save(&dir));
        assert_eq!(Gear::load(&dir), Some(g));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
