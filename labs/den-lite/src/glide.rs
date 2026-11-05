//! The copy in hand glides (owner 5 Nov, after his live test: "objects should ... dynamically move instead of
//! disappearing, reappearing, you need to make sure it makes the video i gave you").
//!
//! A gameobject's place can't change after its create in 3.3.5a (gizmo.rs, the module notes), so until now the copy
//! under the cursor was a gameobject made anew at each new spot (and the old one taken away): about 30 creates a
//! second, which the client draws as a flicker. A creature, though, moves: SMSG_MONSTER_MOVE glides it along a
//! spline, as the far camera's object (farsight.rs) and the companion (flight.rs) already do. So the copy is a
//! creature of ours that wears the prop's own model:
//!
//! * **The model.** A creature's model comes from `CreatureDisplayInfo.dbc` (its display id) and
//!   `CreatureModelData.dbc` (the model's file). The launcher builds the Den's own patch-T from the player's OWN tables
//!   (launcher/src-tauri/src/host/creaturemodels.rs; nothing is shipped) and adds, for every gameobject display whose
//!   model is an `.m2` / `.mdx` (not a building's `.wmo`), a creature display [`DISPLAY_BASE`] + that display id
//!   (and a model row with the same id) naming the same file. Den Lite knows the formula ([`display_of`]).
//! * **Which ones.** The launcher writes [`MARKER`] in the Dens folder with the gameobject displays it covered. Den
//!   Lite glides only those, and only for the Den's owner (the patch is on this PC; a friend's game has no such rows,
//!   so a friend's copy is the gameobject as before). With no marker, or a display not in it, nothing changes.
//! * **Buildings** (WMO models) can't be a creature's model: their copy stays a gameobject made anew, at most every
//!   gizmo.rs `WMO_DRAW_MS` and only when it moved (docs/DEN-EDITOR-PARITY.md).
//! * **The glide.** Made once where the copy starts (flying, no gravity: it stays at the height Den Lite gives it, a
//!   table's top too), then each new spot is a short flying spline from where it is at that moment to the new spot
//!   ([`move_body`], [`GLIDE_MS`]) with its facing fixed (MoveSplineFlag OrientationFixed: it never turns to face the
//!   way it slides) and turned to the copy's turn when it gets there (MonsterMoveType 4, the angle). On the drop the
//!   real gameobject is made first, then the creature goes (never a frame without it).
//!
//! Nothing here can be clicked: the creature is NOT_SELECTABLE (no name, no circle, no tooltip), like the camera's.

use crate::companion::At;
use crate::raw::packed_guid;
use std::collections::HashSet;
use std::path::Path;
use wow_world_messages::wrath::*;
use wow_world_messages::Guid;

/// A gameobject display `d` worn by a creature: creature display (and model data) id `DISPLAY_BASE + d`. Above every
/// id of the game's own tables (3.3.5a's creature displays end near 32 000) and of the Den's own gameobject displays
/// (60 001 ..= 69 999: my models, rebuilt places), so `DISPLAY_BASE + 69 999` is still far from anything.
pub const DISPLAY_BASE: u32 = 70_000;
/// The copy's creature entry, and its two guids (made under the other one when its model changes, before the old one
/// goes: never a frame without a copy).
pub const ENTRY: u32 = 990_030;
const HIGH_CREATURE: u64 = 0xF130;
pub fn guid(flip: bool) -> u64 {
    (HIGH_CREATURE << 48) | ((ENTRY as u64) << 24) | (1 + flip as u64)
}
pub fn is_glider(g: u64) -> bool {
    g == guid(false) || g == guid(true)
}
pub const NAME: &str = "";
/// UNIT_FLAG_NON_ATTACKABLE | IMMUNE_TO_PC | IMMUNE_TO_NPC | NOT_SELECTABLE (farsight.rs FLAGS): nothing can click,
/// target or hit it.
pub const FLAGS: u32 = 0x0000_0002 | 0x0000_0100 | 0x0000_0200 | 0x0200_0000;
/// MoveSplineFlag (3.3.5a, as AzerothCore's MoveSplineFlag): Flying (no ground under it, the path as given) and
/// OrientationFixed (the model keeps its facing while it slides; MonsterMoveType 4 turns it at the end).
pub const SPLINE_FLYING: u32 = 0x0000_2000;
pub const SPLINE_ORIENTATION_FIXED: u32 = 0x0000_4000;
/// MonsterMoveType FacingAngle: an angle follows the type byte.
pub const FACING_ANGLE: u8 = 4;
/// How long each glide to the cursor's newest spot takes (ms). The hovers come about every 33 ms while something is
/// in hand, so each glide is overtaken by the next one from where the copy is then: it follows the cursor smoothly,
/// about this far behind.
pub const GLIDE_MS: u32 = 80;

/// `<dens>/den-creature-models.json` (the launcher, host/creaturemodels.rs `marker`): the gameobject displays the
/// Den's patch-T gives a creature display.
pub const MARKER: &str = "den-creature-models.json";
pub const FORMAT: &str = "leyline-den-creature-models";
/// At most this many displays are read from it (3.3.5a has about 9 000 gameobject displays).
pub const MAX: usize = 40_000;

/// The creature display that wears gameobject display `d`.
pub fn display_of(d: u32) -> Option<u32> {
    d.checked_add(DISPLAY_BASE).filter(|_| d > 0 && d < DISPLAY_BASE)
}

/// The marker's gameobject displays (None: no marker, not ours, another base).
pub fn parse(text: &str) -> Option<HashSet<u32>> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    if v["format"] != FORMAT || v["base"].as_u64() != Some(DISPLAY_BASE as u64) {
        return None;
    }
    let set: HashSet<u32> = v["displays"].as_array()?.iter().take(MAX).filter_map(|d| d.as_u64()).filter_map(|d| u32::try_from(d).ok()).filter(|d| display_of(*d).is_some()).collect();
    Some(set)
}

/// [`parse`] of `<dens>/`[`MARKER`]. A marker that names the archive holding the rows (`archive`: Den Lite's own
/// `glide-rows`, gliderows.rs) counts only while that archive is still in the game's Data folder.
pub fn read(dir: &Path) -> Option<HashSet<u32>> {
    let text = std::fs::read_to_string(dir.join(MARKER)).ok()?;
    if let Some(a) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok().and_then(|v| v["archive"].as_str().map(str::to_string)) {
        if !Path::new(&a).is_file() {
            return None;
        }
    }
    parse(&text)
}

/// Whether the copy of `o` can glide as a creature: its display covered, a model that isn't a building's, upright
/// (a creature can't lean: a tilted prop keeps the gameobject copy), a sane size.
pub fn fits(covered: &HashSet<u32>, o: &crate::den::WorldObject) -> bool {
    covered.contains(&o.display)
        && !crate::models::is_wmo(o.display)
        && o.pitch.abs() < 1e-3
        && o.roll.abs() < 1e-3
        && o.scale.is_finite()
        && o.scale > 0.0
        && [o.x, o.y, o.z, o.o].iter().all(|v| v.is_finite())
}

/// The copy's create: a unit at its spot wearing the prop's model (creature display [`display_of`]), its size the
/// prop's, nobody's to click.
pub fn create(g: u64, go_display: u32, scale: f32, at: At) -> Option<SMSG_UPDATE_OBJECT> {
    let display = display_of(go_display)?;
    let guid = Guid::new(g);
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(guid)
        .set_object_entry(ENTRY as i32)
        .set_object_scale_x(scale)
        .set_unit_bytes_0(Race::Human, Class::Warrior, Gender::Male, Power::Rage)
        .set_unit_health(1)
        .set_unit_maxhealth(1)
        .set_unit_level(1)
        .set_unit_factiontemplate(35)
        .set_unit_flags(FLAGS as i32)
        .set_unit_boundingradius(0.1)
        .set_unit_combatreach(0.1)
        .set_unit_displayid(display as i32)
        .set_unit_nativedisplayid(display as i32)
        .set_unit_bytes_1(UnitStandState::Stand, 0, 0, 0)
        .set_unit_mod_cast_speed(1.0)
        .set_unit_npc_flags(0)
        .set_unit_hoverheight(1.0)
        .finalize();
    let living = MovementBlock_UpdateFlag_Living::Living {
        backwards_flight_speed: 60.0,
        backwards_running_speed: 60.0,
        backwards_swimming_speed: 60.0,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 60.0,
        orientation: at.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: at.x, y: at.y, z: at.z },
        running_speed: 60.0,
        swimming_speed: 60.0,
        timestamp: 0,
        turn_rate: 2.0 * std::f32::consts::PI,
        walking_speed: 60.0,
    };
    Some(SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject {
            guid3: guid,
            mask2: UpdateMask::Unit(mask),
            movement2: MovementBlock { update_flag: MovementBlock_UpdateFlag::empty().set_living(living) },
            object_type: ObjectType::Unit,
        }],
    })
}

/// CMSG_CREATURE_QUERY for the copy: no name (it can't be selected; a name would only show on a nameplate). Its
/// model is the unit's own display field, not this answer's.
pub fn creature(entry: u32) -> Option<SMSG_CREATURE_QUERY_RESPONSE> {
    (entry == ENTRY).then(|| SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: NAME.into(),
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: String::new(),
            description: String::new(),
            type_flags: 0,
            creature_type: 10, // not specified
            creature_family: CreatureFamily::None,
            creature_rank: 0,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [crate::farsight::DISPLAY, 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

/// SMSG_MONSTER_MOVE of the copy from `from` to `to` in `ms` (at least 1): packed guid, a byte 0, the start, the
/// spline id, type 4 and the end's facing, the flags (Flying | OrientationFixed: the path uncompressed, so the one
/// point is written whole, and the model never turns to the way it slides), the time, one point.
pub fn move_body(g: u64, from: [f32; 4], to: [f32; 4], id: u32, ms: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(48);
    packed_guid(&mut b, g);
    b.push(0);
    for v in [from[0], from[1], from[2]] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&id.to_le_bytes());
    b.push(FACING_ANGLE);
    b.extend_from_slice(&to[3].to_le_bytes());
    b.extend_from_slice(&(SPLINE_FLYING | SPLINE_ORIENTATION_FIXED).to_le_bytes());
    b.extend_from_slice(&ms.max(1).to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    for v in [to[0], to[1], to[2]] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

pub fn at(p: [f32; 4]) -> At {
    At { x: p[0], y: p[1], z: p[2], o: p[3] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::WorldObject;

    fn obj(display: u32) -> WorldObject {
        WorldObject { guid: 1, entry: 2413, display, kind: 5, scale: 1.0, x: 1.0, y: 2.0, z: 3.0, o: 0.5, pitch: 0.0, roll: 0.0 }
    }

    #[test]
    fn the_formula_and_the_marker() {
        assert_eq!(display_of(39), Some(70_039));
        assert_eq!(display_of(69_999), Some(139_999));
        assert_eq!(display_of(0), None);
        assert_eq!(display_of(DISPLAY_BASE), None, "never a display of ours mapped onto ours");
        let m = parse(r#"{"format": "leyline-den-creature-models", "version": 1, "base": 70000, "displays": [39, 60001, 0, -4, 70000, "x"]}"#).unwrap();
        assert_eq!(m, [39, 60001].into_iter().collect::<HashSet<u32>>());
        assert!(parse(r#"{"format": "leyline-den-creature-models", "base": 80000, "displays": [39]}"#).is_none(), "another formula: none");
        assert!(parse(r#"{"format": "other", "base": 70000, "displays": [39]}"#).is_none());
        assert!(parse("not json").is_none());
        let d = std::env::temp_dir().join(format!("den-glide-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        assert!(read(&d.join("none")).is_none());
        std::fs::write(d.join(MARKER), r#"{"format": "leyline-den-creature-models", "version": 1, "base": 70000, "displays": [7]}"#).unwrap();
        assert!(read(&d).unwrap().contains(&7));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_a_covered_upright_prop_glides() {
        let set: HashSet<u32> = [39].into_iter().collect();
        assert!(fits(&set, &obj(39)));
        assert!(!fits(&set, &obj(40)), "not in the patch: the gameobject copy");
        assert!(!fits(&set, &WorldObject { pitch: 0.3, ..obj(39) }), "tilted: a creature can't lean");
        assert!(!fits(&set, &WorldObject { scale: f32::NAN, ..obj(39) }));
    }

    #[test]
    fn the_glide_packet_starts_where_it_is_and_ends_turned() {
        let g = guid(false);
        assert!(is_glider(g) && is_glider(guid(true)) && guid(true) != g && !is_glider(crate::farsight::GUID));
        let b = move_body(g, [1.0, 2.0, 3.0, 0.0], [4.0, 5.0, 6.0, 1.5], 77, 0);
        let mut want = Vec::new();
        packed_guid(&mut want, g);
        let n = want.len();
        assert_eq!(&b[..n], &want[..]);
        assert_eq!(b[n], 0);
        let f = |at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let u = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!((f(n + 1), f(n + 5), f(n + 9)), (1.0, 2.0, 3.0), "from where it is now");
        assert_eq!(u(n + 13), 77);
        assert_eq!(b[n + 17], FACING_ANGLE);
        assert_eq!(f(n + 18), 1.5, "turned as the copy");
        assert_eq!(u(n + 22), SPLINE_FLYING | SPLINE_ORIENTATION_FIXED);
        assert_eq!(u(n + 26), 1, "at least 1 ms");
        assert_eq!(u(n + 30), 1, "one point");
        assert_eq!((f(n + 34), f(n + 38), f(n + 42)), (4.0, 5.0, 6.0));
        assert_eq!(b.len(), n + 46);
        assert!(create(g, 39, 1.0, at([0.0; 4])).is_some());
        assert!(create(g, 0, 1.0, at([0.0; 4])).is_none());
        assert!(creature(ENTRY).is_some() && creature(ENTRY + 1).is_none());
    }
}
