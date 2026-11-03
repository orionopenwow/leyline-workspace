//! Pets and real items for the sandbox (owner 3 Nov: "pets (a simple pet that attacks the target) if feasible";
//! "weapons work for weapon-only spells"). Plain message bodies, written as AzerothCore writes them (checked against
//! its Object::BuildMovementUpdate, Unit::BuildMovementPacket, Player::PetSpellInitialize, CharmInfo's pet bar and
//! WorldSession::HandleItemQuerySingleOpcode; the message crate's own reading of SMSG_ITEM_QUERY_SINGLE_RESPONSE):
//!
//! * A pet: a unit create (UPDATETYPE_CREATE_OBJECT, a living movement block, its fields: summoned and created by
//!   you, your faction, player-controlled), your UNIT_FIELD_SUMMON set to it, and SMSG_PET_SPELLS: its bar (Attack,
//!   Follow, Stay; its spell; Aggressive, Defensive, Passive). combat.rs moves it (SMSG_MONSTER_MOVE), lets it follow
//!   you and fight your target, and counts its damage with yours. CMSG_PET_ACTION: its bar's buttons. Dismissed:
//!   SMSG_DESTROY_OBJECT and an empty SMSG_PET_SPELLS.
//! * A weapon: an item object (UPDATETYPE_CREATE_OBJECT, type item, owned and contained by you) in
//!   PLAYER_FIELD_INV_SLOT_HEAD + 2 x slot, and the answer to the game's CMSG_ITEM_QUERY_SINGLE for it (its class and
//!   subclass from the player's own Item.dbc; the name is Den Lite's own: Item.dbc has none). Then the game lets you
//!   cast what needs that weapon.

use crate::raw::packed_guid;

pub const SMSG_PET_SPELLS: u16 = 0x179;
pub const CMSG_PET_ACTION: u32 = 0x175;
pub const SMSG_DESTROY_OBJECT: u16 = 0x0AA;
pub const CMSG_ITEM_QUERY_SINGLE: u32 = 0x056;
pub const SMSG_ITEM_QUERY_SINGLE_RESPONSE: u16 = 0x058;
pub const SMSG_MONSTER_MOVE: u16 = 0x0DD;

/// UNIT_FIELD_SUMMON (your pet), PLAYER_FIELD_INV_SLOT_HEAD (your equipped items' guids).
pub const UNIT_FIELD_SUMMON: u16 = 8;
pub const INV_SLOT_HEAD: u16 = 324;

/// A pet Den Lite knows: the creature entry the summon names, its name, its family (CreatureFamily), its spell (a
/// caster's bolt; 0: it fights in melee).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetDef {
    pub entry: u32,
    pub name: &'static str,
    pub family: u16,
    pub bolt: u32,
}

/// The hunter's pet when Call Pet names none (a wolf).
pub const HUNTER_PET: u32 = 69;
/// The death knight's ghoul (Raise Dead names it by script, not by its effect).
pub const GHOUL: u32 = 26125;

pub const PETS: &[PetDef] = &[
    PetDef { entry: HUNTER_PET, name: "Wolf", family: 1, bolt: 0 },
    PetDef { entry: 416, name: "Imp", family: 23, bolt: 47964 },
    PetDef { entry: 1860, name: "Voidwalker", family: 16, bolt: 0 },
    PetDef { entry: 1863, name: "Succubus", family: 17, bolt: 0 },
    PetDef { entry: 417, name: "Felhunter", family: 15, bolt: 0 },
    PetDef { entry: 17252, name: "Felguard", family: 29, bolt: 0 },
    PetDef { entry: 510, name: "Water Elemental", family: 0, bolt: 31707 },
    PetDef { entry: GHOUL, name: "Risen Ghoul", family: 0, bolt: 0 },
    PetDef { entry: 29264, name: "Spirit Wolf", family: 0, bolt: 0 },
];

pub fn pet_def(entry: u32) -> Option<&'static PetDef> {
    PETS.iter().find(|p| p.entry == entry)
}

/// The model a pet shows: the creature's own (creatures.rs, from AzerothCore's world database), else the training
/// dummy's.
pub fn pet_display(entry: u32) -> u32 {
    crate::creatures::display(entry).unwrap_or(27510)
}

/// A pet's guid (HighGuid::Pet 0xF140, the entry, a counter).
pub fn pet_guid(entry: u32, n: u32) -> u64 {
    (0xF140u64 << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (n as u64 & 0xFF_FFFF)
}

/// An item's guid (HighGuid::Item 0x4000, a counter).
pub fn item_guid(n: u32) -> u64 {
    (0x4000u64 << 48) | n as u64
}

/// The update mask and the values (sorted, one u32 each), as a values block writes them.
fn mask_values(b: &mut Vec<u8>, fields: &[(u16, u32)]) {
    let mut f = fields.to_vec();
    f.sort_by_key(|x| x.0);
    f.dedup_by_key(|x| x.0);
    let max = f.last().map(|x| x.0).unwrap_or(0) as usize;
    let blocks = max / 32 + 1;
    let mut mask = vec![0u32; blocks];
    for (i, _) in &f {
        mask[*i as usize / 32] |= 1 << (i % 32);
    }
    b.push(blocks as u8);
    for m in mask {
        b.extend_from_slice(&m.to_le_bytes());
    }
    for (_, v) in f {
        b.extend_from_slice(&v.to_le_bytes());
    }
}

fn guid_fields(at: u16, g: u64) -> [(u16, u32); 2] {
    [(at, g as u32), (at + 1, (g >> 32) as u32)]
}

/// SMSG_UPDATE_OBJECT creating a unit at x y z o (UPDATEFLAG_LIVING: its movement standing, the speeds), with
/// `fields` besides its guid and type.
pub fn create_unit_body(guid: u64, at: [f32; 4], time_ms: u32, fields: &[(u16, u32)]) -> Vec<u8> {
    let mut b = Vec::with_capacity(200);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(2); // UPDATETYPE_CREATE_OBJECT
    packed_guid(&mut b, guid);
    b.push(3); // TYPEID_UNIT
    b.extend_from_slice(&0x20u16.to_le_bytes()); // UPDATEFLAG_LIVING
    b.extend_from_slice(&0u32.to_le_bytes()); // movement flags
    b.extend_from_slice(&0u16.to_le_bytes()); // extra flags
    b.extend_from_slice(&time_ms.to_le_bytes());
    for v in at {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&0u32.to_le_bytes()); // fall time
    // walk, run, run back, swim, swim back, flight, flight back, turn rate, pitch rate
    for v in [2.5f32, 7.0, 4.5, 4.722222, 2.5, 7.0, 4.5, std::f32::consts::PI, std::f32::consts::PI] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let mut f: Vec<(u16, u32)> = guid_fields(0, guid).to_vec();
    f.push((2, 0x9)); // TYPEMASK_OBJECT | TYPEMASK_UNIT
    f.push((4, 1.0f32.to_bits())); // scale
    f.extend_from_slice(fields);
    mask_values(&mut b, &f);
    b
}

/// A pet's fields: its entry, summoned and created by `owner`, its health and level, your faction (1), class
/// warrior with mana, player-controlled, its model, its number.
pub fn pet_fields(entry: u32, owner: u64, level: u32, health: u32, display: u32) -> Vec<(u16, u32)> {
    let mut f = vec![(3, entry)];
    f.extend(guid_fields(14, owner)); // UNIT_FIELD_SUMMONEDBY
    f.extend(guid_fields(16, owner)); // UNIT_FIELD_CREATEDBY
    f.extend([
        (23, 1 << 8),         // UNIT_FIELD_BYTES_0: race 0, class 1, gender 0, power 0
        (24, health),         // UNIT_FIELD_HEALTH
        (32, health),         // UNIT_FIELD_MAXHEALTH
        (54, level),          // UNIT_FIELD_LEVEL
        (55, 1),              // UNIT_FIELD_FACTIONTEMPLATE: yours
        (59, 0x8),            // UNIT_FIELD_FLAGS: UNIT_FLAG_PLAYER_CONTROLLED
        (62, 2000),           // UNIT_FIELD_BASEATTACKTIME
        (65, 0.5f32.to_bits()), // bounding radius
        (66, 1.5f32.to_bits()), // combat reach
        (67, display),
        (68, display),
        (75, 1),              // UNIT_FIELD_PETNUMBER
        (80, 1.0f32.to_bits()), // UNIT_MOD_CAST_SPEED
        (146, 1.0f32.to_bits()), // UNIT_FIELD_HOVERHEIGHT
    ]);
    f
}

/// SMSG_PET_SPELLS: the pet's guid, family, duration 0, react state, command state, u16 0, its bar (10 buttons:
/// Attack, Follow, Stay (ACT_COMMAND 7), its spells (autocast on: 0xC1; none: 0 passive), Aggressive, Defensive,
/// Passive (ACT_REACTION 6)), its spells (the bar's), no cooldowns.
pub fn pet_spells_body(pet: u64, family: u16, react: u8, command: u8, spells: &[u32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(64);
    b.extend_from_slice(&pet.to_le_bytes());
    b.extend_from_slice(&family.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.push(react);
    b.push(command);
    b.extend_from_slice(&0u16.to_le_bytes());
    let button = |a: u32, t: u32| a | t << 24;
    let mut bar = vec![button(2, 7), button(1, 7), button(0, 7)];
    for k in 0..4 {
        bar.push(spells.get(k).map(|s| button(*s, 0xC1)).unwrap_or(button(0, 0x01)));
    }
    bar.extend([button(2, 6), button(1, 6), button(0, 6)]);
    for v in &bar {
        b.extend_from_slice(&v.to_le_bytes());
    }
    let sp: Vec<u32> = spells.iter().take(4).map(|s| button(*s, 0xC1)).collect();
    b.push(sp.len() as u8);
    for v in sp {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.push(0);
    b
}

/// SMSG_PET_SPELLS with no pet: the bar goes.
pub fn no_pet_body() -> Vec<u8> {
    0u64.to_le_bytes().to_vec()
}

/// SMSG_DESTROY_OBJECT: the guid, "died" 0.
pub fn destroy_body(guid: u64) -> Vec<u8> {
    let mut b = guid.to_le_bytes().to_vec();
    b.push(0);
    b
}

/// CMSG_PET_ACTION: (pet, the button's action, its type, the target).
pub fn read_pet_action(body: &[u8]) -> Option<(u64, u32, u8, u64)> {
    let pet = u64::from_le_bytes(body.get(..8)?.try_into().ok()?);
    let data = u32::from_le_bytes(body.get(8..12)?.try_into().ok()?);
    let target = body.get(12..20).and_then(|s| s.try_into().ok()).map(u64::from_le_bytes).unwrap_or(0);
    Some((pet, data & 0x00FF_FFFF, (data >> 24) as u8, target))
}

/// SMSG_MONSTER_MOVE: a unit walks (MonsterMoveType 0) from `from` to `to` in `ms`.
pub fn move_body(guid: u64, from: [f32; 3], to: [f32; 3], id: u32, ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, guid);
    b.push(0);
    for v in from {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&id.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&ms.max(1).to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    for v in to {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

// ---------------------------------------------------------------- weapons as real items

/// SMSG_UPDATE_OBJECT creating an item (UPDATETYPE_CREATE_OBJECT, TYPEID_ITEM, no movement): its entry, owned and
/// contained by `owner`, one of it, full durability.
pub fn create_item_body(guid: u64, entry: u32, owner: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(96);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.push(2);
    packed_guid(&mut b, guid);
    b.push(1); // TYPEID_ITEM
    b.extend_from_slice(&0u16.to_le_bytes());
    let mut f: Vec<(u16, u32)> = guid_fields(0, guid).to_vec();
    f.push((2, 0x3)); // TYPEMASK_OBJECT | TYPEMASK_ITEM
    f.push((3, entry));
    f.push((4, 1.0f32.to_bits()));
    f.extend(guid_fields(6, owner)); // ITEM_FIELD_OWNER
    f.extend(guid_fields(8, owner)); // ITEM_FIELD_CONTAINED
    f.push((14, 1)); // ITEM_FIELD_STACK_COUNT
    f.push((60, 100)); // ITEM_FIELD_DURABILITY
    f.push((61, 100)); // ITEM_FIELD_MAXDURABILITY
    mask_values(&mut b, &f);
    b
}

/// The player's fields for the items in their slots (slot -> item guid; 0: empty).
pub fn inv_fields(slots: &[(u8, u64)]) -> Vec<(u16, u32)> {
    slots.iter().flat_map(|&(s, g)| guid_fields(INV_SLOT_HEAD + 2 * s as u16, g)).collect()
}

/// A Den Lite name for an item (Item.dbc has none): "Dojo " and its kind.
pub fn item_name(class: u32, subclass: u32) -> String {
    let kind = match (class, subclass) {
        (2, 0) => "Axe",
        (2, 1) => "Great Axe",
        (2, 2) => "Bow",
        (2, 3) => "Gun",
        (2, 4) => "Mace",
        (2, 5) => "Great Mace",
        (2, 6) => "Polearm",
        (2, 7) => "Sword",
        (2, 8) => "Greatsword",
        (2, 10) => "Staff",
        (2, 13) => "Fist Weapon",
        (2, 15) => "Dagger",
        (2, 16) => "Thrown",
        (2, 18) => "Crossbow",
        (2, 19) => "Wand",
        (4, 6) => "Shield",
        (2, _) => "Weapon",
        _ => "Gear",
    };
    format!("Dojo {kind}")
}

/// SMSG_ITEM_QUERY_SINGLE_RESPONSE for an item Den Lite made real (3.3.5a, as the message crate reads it): its
/// entry, class and subclass, the name, its model, epic, its inventory type, item level 200, no requirements, its
/// damage (min, max, physical) and swing time (ms), nothing else.
pub fn item_query_body(entry: u32, class: u32, subclass: u32, display: u32, inv: u32, min: f32, max: f32, delay: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(512);
    let u = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
    u(&mut b, entry);
    u(&mut b, class);
    u(&mut b, subclass);
    u(&mut b, u32::MAX); // sound override: -1
    b.extend_from_slice(item_name(class, subclass).as_bytes());
    b.extend_from_slice(&[0, 0, 0, 0]); // the name's end, three empty names
    u(&mut b, display);
    u(&mut b, 4); // epic
    u(&mut b, 0); // flags
    u(&mut b, 0); // flags 2
    u(&mut b, 0); // buy
    u(&mut b, 0); // sell
    u(&mut b, inv);
    u(&mut b, u32::MAX); // any class
    u(&mut b, u32::MAX); // any race
    u(&mut b, 200); // item level
    for _ in 0..9 {
        u(&mut b, 0); // level, skill, skill rank, spell, honor rank, city rank, faction, faction rank, max count
    }
    u(&mut b, 1); // stackable
    u(&mut b, 0); // container slots
    u(&mut b, 0); // no stats
    u(&mut b, 0); // scaling stats entry
    u(&mut b, 0); // scaling stats flag
    let weapon = class == 2;
    for k in 0..2 {
        let (lo, hi) = if weapon && k == 0 { (min, max) } else { (0.0, 0.0) };
        b.extend_from_slice(&lo.to_le_bytes());
        b.extend_from_slice(&hi.to_le_bytes());
        u(&mut b, 0); // physical
    }
    u(&mut b, if class == 4 && subclass == 6 { 5000 } else { 0 }); // armour (a shield's)
    for _ in 0..6 {
        u(&mut b, 0); // resistances
    }
    u(&mut b, if weapon { delay } else { 0 });
    u(&mut b, 0); // ammo
    b.extend_from_slice(&0f32.to_le_bytes()); // ranged range
    for _ in 0..5 {
        for _ in 0..6 {
            u(&mut b, 0); // no spells
        }
    }
    u(&mut b, 0); // bonding
    b.push(0); // no description
    for _ in 0..7 {
        u(&mut b, 0); // page text, language, page material, start quest, lock, material, sheath
    }
    for _ in 0..3 {
        u(&mut b, 0); // random property, random suffix, block
    }
    u(&mut b, 0); // item set
    u(&mut b, 100); // max durability
    for _ in 0..4 {
        u(&mut b, 0); // area, map, bag family, totem category
    }
    for _ in 0..6 {
        u(&mut b, 0); // three sockets
    }
    for _ in 0..3 {
        u(&mut b, 0); // socket bonus, gem properties, disenchant skill
    }
    b.extend_from_slice(&0f32.to_le_bytes()); // armour damage modifier
    for _ in 0..3 {
        u(&mut b, 0); // duration, limit category, holiday
    }
    b
}

/// CMSG_ITEM_QUERY_SINGLE: the entry asked about.
pub fn read_item_query(body: &[u8]) -> Option<u32> {
    body.get(..4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pets_create_its_bar_and_its_actions() {
        let me = 1u64;
        let g = pet_guid(416, 1);
        assert_eq!(g >> 48, 0xF140);
        let body = create_unit_body(g, [1.0, 2.0, 3.0, 0.5], 1234, &pet_fields(416, me, 80, 8000, 4449));
        assert_eq!(&body[..5], &[1, 0, 0, 0, 2]);
        let mut at = 5;
        assert_eq!(crate::combat::read_packed(&body, &mut at), Some(g));
        assert_eq!(body[at], 3, "a unit");
        assert_eq!(&body[at + 1..at + 3], &[0x20, 0], "living");
        let pos = at + 3 + 4 + 2 + 4;
        assert_eq!(f32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()), 1.0);
        // the values: mask blocks up to field 146 (5 blocks)
        let v = pos + 16 + 4 + 36;
        assert_eq!(body[v], 5);
        let f = pet_fields(416, me, 80, 8000, 4449);
        assert!(f.contains(&(14, 1)) && f.contains(&(16, 1)) && f.contains(&(55, 1)) && f.contains(&(67, 4449)));
        let s = pet_spells_body(g, 23, 1, 1, &[47964]);
        assert_eq!(s.len(), 8 + 2 + 4 + 1 + 1 + 2 + 40 + 1 + 4 + 1);
        assert_eq!(u32::from_le_bytes(s[18..22].try_into().unwrap()), 2 | 7 << 24, "Attack first");
        assert_eq!(u32::from_le_bytes(s[30..34].try_into().unwrap()), 47964 | 0xC1 << 24, "its bolt, autocast");
        assert_eq!(u32::from_le_bytes(s[46..50].try_into().unwrap()), 2 | 6 << 24, "Aggressive");
        assert_eq!(no_pet_body(), vec![0; 8]);
        let mut act = g.to_le_bytes().to_vec();
        act.extend_from_slice(&(2u32 | 7 << 24).to_le_bytes());
        act.extend_from_slice(&77u64.to_le_bytes());
        assert_eq!(read_pet_action(&act), Some((g, 2, 7, 77)));
        assert_eq!(read_pet_action(&act[..6]), None);
        assert_eq!(destroy_body(5), vec![5, 0, 0, 0, 0, 0, 0, 0, 0]);
        let m = move_body(g, [0.0; 3], [1.0, 0.0, 0.0], 9, 500);
        assert_eq!(m.len(), 6 + 1 + 12 + 4 + 1 + 4 + 4 + 4 + 12, "a packed guid of 6");
        for p in PETS {
            assert!(!crate::companion::branded(p.name));
        }
        assert_eq!(pet_def(416).map(|p| p.name), Some("Imp"));
    }

    #[test]
    fn a_weapon_is_a_real_item_the_game_can_ask_about() {
        let g = item_guid(7);
        assert_eq!(g >> 48, 0x4000);
        let body = create_item_body(g, 19019, 1);
        let mut at = 5;
        assert_eq!(crate::combat::read_packed(&body, &mut at), Some(g));
        assert_eq!((body[at], body[at + 1], body[at + 2]), (1, 0, 0), "an item, no movement");
        assert_eq!(body[at + 3], 2, "two mask blocks (up to field 61)");
        assert_eq!(inv_fields(&[(15, g)]), vec![(354, 7), (355, 0x4000_0000)]);
        let q = item_query_body(19019, 2, 7, 30606, 13, 100.0, 200.0, 2600);
        assert_eq!(&q[..12], &[0x4B, 0x4A, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0]);
        assert!(q.windows(10).any(|w| w == b"Dojo Sword"));
        assert_eq!(read_item_query(&19019u32.to_le_bytes()), Some(19019));
        // the fixed part: 4 * 3 + 4 + the name and 4 zeros + the rest
        let name = "Dojo Sword".len() + 4;
        // display..item level 40, requirements 36, stack/slots/stats 12, scaling 8, damages 24, armour and resistances
        // 28, delay/ammo/range 12, spells 120, bonding 4, description 1, page..sheath 28, random/block 12, set 4,
        // durability 4, area..totem 16, sockets 24, bonus/gem/disenchant 12, modifier 4, duration..holiday 12
        let rest = 40 + 36 + 12 + 8 + 24 + 28 + 12 + 120 + 4 + 1 + 28 + 12 + 4 + 4 + 16 + 24 + 12 + 4 + 12;
        assert_eq!(q.len(), 16 + name + rest, "every field of the 3.3.5a answer");
        assert_eq!(item_name(4, 6), "Dojo Shield");
    }
}
