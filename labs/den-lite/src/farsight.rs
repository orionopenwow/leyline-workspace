//! The far camera (owner 3 Nov: "the 'snap out' camera mode working for sure too and infinite camera zoom out from
//! character").
//!
//! The 3.3.5a client's own camera stays within its zoom limit (the CVars `cameraDistanceMax` and
//! `cameraDistanceMaxFactor`, at most about 50 yards) of whatever it watches. What it watches is the player, unless
//! the player's `PLAYER_FARSIGHT` field holds another object's guid: then the camera goes to that object (as the
//! game's own Far Sight, Mind Vision and Eagle Eye do) and the player stays where they are. So for the editor's
//! overview (snap out) and for zooming out past the limit, Den Lite makes one object of its own, an invisible unit
//! nobody can click ([`create`]), puts it where the camera should look from (gizmo.rs `far_spot`: back along the
//! build camera's own line of sight, so the middle of the screen stays on the same ground), moves it with a short
//! flying spline when the view pans or zooms ([`move_body`]), and sets far sight on it (raw.rs `farsight`); to
//! come back, far sight 0 and the object goes. Den Lite always knows where that object is, so the cursor's ray
//! (gizmo.rs `Aids::view`) stays exact from up there.

use crate::companion::At;
use crate::flight;
use wow_world_messages::wrath::*;
use wow_world_messages::Guid;

/// The camera object's entry and guid (a creature of ours; its guid never meets a prop's or the companion's).
pub const ENTRY: u32 = 990_020;
pub const GUID: u64 = (0xF130u64 << 48) | ((ENTRY as u64) << 24) | 1;
/// An invisible model (the "invisible stalker" trigger model the game's own triggers use).
pub const DISPLAY: u32 = 11686;
pub const NAME: &str = "Camera";
/// UNIT_FLAG_NON_ATTACKABLE | IMMUNE_TO_PC | IMMUNE_TO_NPC | NOT_SELECTABLE: nothing can click, target or hit it.
pub const FLAGS: u32 = 0x0000_0002 | 0x0000_0100 | 0x0000_0200 | 0x0200_0000;
/// How long a pan or zoom's move takes (ms): short, so the view glides rather than jumps.
pub const MOVE_MS: u32 = 180;

/// The camera object's create: an invisible unit at `at`, nobody's to click.
pub fn create(at: At) -> SMSG_UPDATE_OBJECT {
    let guid = Guid::new(GUID);
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(guid)
        .set_object_entry(ENTRY as i32)
        .set_object_scale_x(1.0)
        .set_unit_bytes_0(Race::Human, Class::Warrior, Gender::Male, Power::Rage)
        .set_unit_health(1)
        .set_unit_maxhealth(1)
        .set_unit_level(1)
        .set_unit_factiontemplate(35)
        .set_unit_flags(FLAGS as i32)
        .set_unit_boundingradius(0.1)
        .set_unit_combatreach(0.1)
        .set_unit_displayid(DISPLAY as i32)
        .set_unit_nativedisplayid(DISPLAY as i32)
        .set_unit_bytes_1(UnitStandState::Stand, 0, 0, 0)
        .set_unit_mod_cast_speed(1.0)
        .set_unit_npc_flags(0)
        .set_unit_hoverheight(1.0)
        .finalize();
    let living = MovementBlock_UpdateFlag_Living::Living {
        backwards_flight_speed: 30.0,
        backwards_running_speed: 30.0,
        backwards_swimming_speed: 30.0,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 60.0,
        orientation: at.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: at.x, y: at.y, z: at.z },
        running_speed: 30.0,
        swimming_speed: 30.0,
        timestamp: 0,
        turn_rate: std::f32::consts::PI,
        walking_speed: 30.0,
    };
    SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject {
            guid3: guid,
            mask2: UpdateMask::Unit(mask),
            movement2: MovementBlock { update_flag: MovementBlock_UpdateFlag::empty().set_living(living) },
            object_type: ObjectType::Unit,
        }],
    }
}

/// CMSG_CREATURE_QUERY for the camera object: its name and model (never shown: it can't be selected).
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
            display_ids: [DISPLAY, 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

pub fn at(p: [f32; 4]) -> At {
    At { x: p[0], y: p[1], z: p[2], o: p[3] }
}

/// SMSG_MONSTER_MOVE of the camera object from `from` to `to`, flying (no ground under it), in [`MOVE_MS`].
pub fn move_body(from: [f32; 4], to: [f32; 4], id: u32) -> Vec<u8> {
    flight::sky_body(GUID, at(from), &[at(to)], id, flight::SPLINE_FLYING, MOVE_MS)
}

/// The free flight's move (gizmo.rs `fly`): the same flying spline, as long as asked (at least 1 ms).
pub fn glide_body(from: [f32; 4], to: [f32; 4], id: u32, ms: u32) -> Vec<u8> {
    flight::sky_body(GUID, at(from), &[at(to)], id, flight::SPLINE_FLYING, ms.max(1))
}
