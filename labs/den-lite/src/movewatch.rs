//! Does the player's game move its character, and does it stand on the ground? (Owner 3 Nov, with screenshots: in
//! the Den the character floated above its own shadow on the white ground and wouldn't walk.)
//!
//! A game that has its character in hand moves it itself: it falls onto the ground, walks, and tells the server
//! where it went (`MSG_MOVE_*`). A character that hangs in the air *and* won't walk is one the game isn't moving at
//! all (no control of it, or rooted), so it never fell the yard Den Lite used to put it above the ground. Two
//! things follow (world.rs):
//!
//! * the player comes in **on** the ground ([`SPAWN_LIFT`] above it, was a whole yard), so even a game that doesn't
//!   move its character shows it standing, never floating;
//! * Den Lite says that the game has its character (SMSG_CLIENT_CONTROL_UPDATE, and an unroot) when the game says it
//!   is in the world (`CMSG_SET_ACTIVE_MOVER`); none during the loading screen (AzerothCore sends none at login);
//!
//! and this watch writes down, per session, what the game did with its character, for the events log (events.rs:
//! `spawn`, `mover`, `first_move`, `move_flags`, `landed`, `walked`) and `.den where`, so the den-test on the owner's PC
//! says which of these it is.

use crate::companion::At;
use std::collections::BTreeMap;

/// How far above the ground the player comes in: on it, a hair above so rounding never puts the feet under it.
pub const SPAWN_LIFT: f32 = 0.05;
/// Standing on the ground: the feet within this of the ground's height.
pub const ON_GROUND: f32 = 0.5;
/// Walked: this far (yards, on the flat) from where the player came in.
pub const WALKED: f32 = 1.0;

/// The game says it moves this character now (it sends this once the world is on screen).
pub const CMSG_SET_ACTIVE_MOVER: u32 = 0x26A;
/// The game's answer to the server's time sync.
pub const CMSG_TIME_SYNC_RESP: u32 = 0x391;
/// Tells the game whether it controls a unit (packed guid, u8 allow): AzerothCore's Player::SetClientControl.
pub const SMSG_CLIENT_CONTROL_UPDATE: u16 = 0x159;
/// Takes a root off (packed guid, u32 counter): AzerothCore's Unit::SetRooted(false) for a player.
pub const SMSG_FORCE_MOVE_UNROOT: u16 = 0x0EA;
pub const MSG_MOVE_FALL_LAND: u32 = 0x0C9;
/// The server's time sync (u32 counter): the game answers with CMSG_TIME_SYNC_RESP (counter, its own clock in ms).
pub const SMSG_TIME_SYNC_REQ: u16 = 0x390;

// The player's create (3 Nov, still no movement at all): as AzerothCore builds it. Every unit carries
// UPDATEFLAG_LIVING | UPDATEFLAG_STATIONARY_POSITION (Unit::Unit, Unit.cpp: `m_updateFlag = (UPDATEFLAG_LIVING |
// UPDATEFLAG_STATIONARY_POSITION)`), and the player's own copy adds UPDATEFLAG_SELF
// (Object::BuildCreateUpdateBlockForPlayer: `if (target == this) flags |= UPDATEFLAG_SELF`). Den Lite sent 0x21.
pub const UPDATEFLAG_SELF: u16 = 0x0001;
pub const UPDATEFLAG_LIVING: u16 = 0x0020;
pub const UPDATEFLAG_STATIONARY_POSITION: u16 = 0x0040;
/// The update flags of a player's create: its own (`own`) or someone else's.
pub const fn player_update_flags(own: bool) -> u16 {
    (if own { UPDATEFLAG_SELF } else { 0 }) | UPDATEFLAG_LIVING | UPDATEFLAG_STATIONARY_POSITION
}

/// The server's clock for movement timestamps: milliseconds since Den Lite started (AzerothCore's
/// GameTime::GetGameTimeMS, which Unit::BuildMovementPacket writes as the movement's time). Den Lite wrote 0. Starts at
/// 1000 so it is never 0.
pub fn server_ms() -> u32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let s = START.get_or_init(std::time::Instant::now);
    (s.elapsed().as_millis() as u64 + 1000).min(u32::MAX as u64) as u32
}

/// SMSG_TIME_SYNC_REQ's body.
pub fn time_sync_body(counter: u32) -> Vec<u8> {
    counter.to_le_bytes().to_vec()
}

/// CMSG_TIME_SYNC_RESP's body: (counter, the game's clock in ms).
pub fn time_sync_resp(body: &[u8]) -> Option<(u32, u32)> {
    let b = body.get(..8)?;
    Some((u32::from_le_bytes([b[0], b[1], b[2], b[3]]), u32::from_le_bytes([b[4], b[5], b[6], b[7]])))
}

/// How many different opcodes from the game are written down once in the world (the `client_op` events).
pub const OPS_KEPT: usize = 48;

/// A client opcode's name for the log (the ones that tell what the game does with its character), else its number.
pub fn client_op_name(op: u32) -> String {
    let n = match op {
        0x0B5..=0x0C3 | 0x0C9..=0x0CB | 0x0DA | 0x0DB | 0x0EE => return format!("MSG_MOVE_{}", op_name(op)),
        0x026A => "SET_ACTIVE_MOVER",
        0x0391 => "TIME_SYNC_RESP",
        0x02CE => "MOVE_TIME_SKIPPED",
        0x00EB => "FORCE_MOVE_UNROOT_ACK",
        0x00E9 => "FORCE_MOVE_ROOT_ACK",
        0x02CA => "MOVE_FALL_RESET",
        0x01DC => "PING",
        0x0095 => "MESSAGECHAT",
        0x020A => "REQUEST_ACCOUNT_DATA",
        0x020B => "UPDATE_ACCOUNT_DATA",
        0x013D => "SET_SELECTION",
        0x01F4 => "ZONEUPDATE",
        0x0050 => "NAME_QUERY",
        0x005E => "GAMEOBJECT_QUERY",
        0x0060 => "CREATURE_QUERY",
        0x04FF => "READY_FOR_ACCOUNT_DATA_TIMES",
        0x02D1 => "MOVE_NOT_ACTIVE_MOVER",
        0x0346 => "MOVE_SET_FLY",
        _ => return format!("{op:#05x}"),
    };
    n.to_string()
}

// 3.3.5 movement flags (the u32 `flags` of a MSG_MOVE_* packet), the ones that keep a character from falling or walking
pub const FORWARD: u32 = 0x1;
pub const ON_TRANSPORT: u32 = 0x200;
pub const DISABLE_GRAVITY: u32 = 0x400;
pub const ROOT: u32 = 0x800;
pub const FALLING: u32 = 0x1000;
pub const SWIMMING: u32 = 0x20_0000;
pub const CAN_FLY: u32 = 0x100_0000;
pub const FLYING: u32 = 0x200_0000;
pub const WATERWALKING: u32 = 0x1000_0000;
pub const FALLING_SLOW: u32 = 0x2000_0000;
pub const HOVER: u32 = 0x4000_0000;
const NOTABLE: &[(u32, &str)] = &[
    (ROOT, "ROOT"),
    (DISABLE_GRAVITY, "DISABLE_GRAVITY"),
    (HOVER, "HOVER"),
    (FLYING, "FLYING"),
    (CAN_FLY, "CAN_FLY"),
    (FALLING, "FALLING"),
    (FALLING_SLOW, "FALLING_SLOW"),
    (SWIMMING, "SWIMMING"),
    (WATERWALKING, "WATERWALKING"),
    (ON_TRANSPORT, "ON_TRANSPORT"),
];

/// The names of the flags that keep a character up or still (empty: none of them).
pub fn flag_names(flags: u32) -> String {
    NOTABLE.iter().filter(|(b, _)| flags & b != 0).map(|(_, n)| *n).collect::<Vec<_>>().join(" ")
}

/// A movement packet's name, for the log.
pub fn op_name(op: u32) -> &'static str {
    match op {
        0x0B5 => "START_FORWARD",
        0x0B6 => "START_BACKWARD",
        0x0B7 => "STOP",
        0x0B8 => "START_STRAFE_LEFT",
        0x0B9 => "START_STRAFE_RIGHT",
        0x0BA => "STOP_STRAFE",
        0x0BB => "JUMP",
        0x0BC => "START_TURN_LEFT",
        0x0BD => "START_TURN_RIGHT",
        0x0BE => "STOP_TURN",
        0x0BF => "START_PITCH_UP",
        0x0C0 => "START_PITCH_DOWN",
        0x0C1 => "STOP_PITCH",
        0x0C2 => "SET_RUN_MODE",
        0x0C3 => "SET_WALK_MODE",
        0x0C9 => "FALL_LAND",
        0x0CA => "START_SWIM",
        0x0CB => "STOP_SWIM",
        0x0DA => "SET_FACING",
        0x0DB => "SET_PITCH",
        0x0EE => "HEARTBEAT",
        _ => "MOVE",
    }
}

/// The `flags` of a movement packet the game sent (packed guid, then u32 flags), read by hand like world.rs
/// `raw_move` (only call it for a packet `raw_move` read).
pub fn raw_flags(body: &[u8]) -> Option<u32> {
    let at = 1 + body.first()?.count_ones() as usize;
    body.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// SMSG_CLIENT_CONTROL_UPDATE's body: the game controls `guid` (allow 1).
pub fn control_body(guid: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(10);
    crate::raw::packed_guid(&mut b, guid);
    b.push(1);
    b
}

/// SMSG_FORCE_MOVE_UNROOT's body: `guid`, the counter the game echoes in its answer.
pub fn unroot_body(guid: u64, counter: u32) -> Vec<u8> {
    let mut b = Vec::with_capacity(13);
    crate::raw::packed_guid(&mut b, guid);
    b.extend_from_slice(&counter.to_le_bytes());
    b
}

fn flat(a: At, b: At) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Where the feet are against the ground: "on the ground", "1.20 above the ground", "0.80 under the ground".
pub fn stand(z: f32, ground: f32) -> String {
    let d = z - ground;
    if d.abs() <= ON_GROUND {
        format!("on the ground ({d:+.2})")
    } else if d > 0.0 {
        format!("{d:.2} above the ground")
    } else {
        format!("{:.2} under the ground", -d)
    }
}

/// The `spawn` event's detail: where the player came in against the ground under it, where that height came from
/// and how near the nearest prop is (a piece standing where the player comes in can hold the character fast).
pub fn spawn_note(at: At, ground: f32, from_ground_file: bool, nearest_prop: Option<f32>) -> String {
    format!(
        "z {:.2}, ground {:.2} ({}), {}, nearest prop {}",
        at.z,
        ground,
        if from_ground_file { "the Den's ground file" } else { "the fixed floor: no ground file" },
        stand(at.z, ground),
        nearest_prop.map(|d| format!("{d:.1} yards")).unwrap_or_else(|| "none".into())
    )
}

/// What one session's game did with its character.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Watch {
    pub sess: u64,
    pub spawn: At,
    /// The ground's height where the player came in.
    pub ground: f32,
    /// The game sent CMSG_SET_ACTIVE_MOVER.
    pub mover: bool,
    /// Movement packets from the game.
    pub moves: u32,
    /// The first one's opcode.
    pub first: Option<u32>,
    /// Every flag seen in its movement packets.
    pub flags_seen: u32,
    /// The first landing (FALL_LAND): z and the ground there.
    pub landed: Option<(f32, f32)>,
    /// How far from the spawn the player was when first seen to have walked.
    pub walked: Option<f32>,
    /// The last position and the ground there.
    pub last: Option<(At, f32)>,
    /// Every opcode the game sent since it came in, and how often (the first [`OPS_KEPT`] different ones).
    pub ops: BTreeMap<u32, u32>,
    /// The game's first answer to the time sync: (counter, its clock).
    pub time_sync: Option<(u32, u32)>,
}

impl Watch {
    pub fn new(sess: u64, spawn: At, ground: f32) -> Watch {
        Watch { sess, spawn, ground, ..Default::default() }
    }

    /// CMSG_SET_ACTIVE_MOVER: true the first time (Den Lite then tells the game once more that it has control).
    pub fn mover(&mut self) -> bool {
        !std::mem::replace(&mut self.mover, true)
    }

    /// A movement packet (`op`, its `flags`, where the character is now, the ground's height there): the events it
    /// makes, as (name, detail).
    pub fn moved(&mut self, op: u32, flags: u32, at: At, ground: f32) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        self.moves += 1;
        if self.first.is_none() {
            self.first = Some(op);
            let f = flag_names(flags);
            out.push(("first_move", format!("{}{}, z {:.2}, {}", op_name(op), if f.is_empty() { String::new() } else { format!(" [{f}]") }, at.z, stand(at.z, ground))));
        }
        let new = NOTABLE.iter().filter(|(b, _)| flags & b != 0 && self.flags_seen & b == 0).fold(0, |a, (b, _)| a | b);
        self.flags_seen |= flags;
        // falling comes and goes with every jump: only the flags that hold a character up or still are told
        if new & !(FALLING) != 0 {
            out.push(("move_flags", format!("{} in {}", flag_names(new & !FALLING), op_name(op))));
        }
        if op == MSG_MOVE_FALL_LAND && self.landed.is_none() {
            self.landed = Some((at.z, ground));
            out.push(("landed", format!("z {:.2}, {}", at.z, stand(at.z, ground))));
        }
        let d = flat(at, self.spawn);
        if self.walked.is_none() && d >= WALKED {
            self.walked = Some(d);
            out.push(("walked", format!("{d:.1} yards by {}, z {:.2}, {}", op_name(op), at.z, stand(at.z, ground))));
        }
        self.last = Some((at, ground));
        out
    }

    /// The character's feet on the ground at the last move (None: no move yet).
    pub fn on_ground(&self) -> Option<bool> {
        self.last.map(|(a, g)| (a.z - g).abs() <= ON_GROUND)
    }

    /// A frame from the game while in the world: Some((name, count)) the first time this opcode is seen (the
    /// `client_op` event), so the log says everything the game sent, not only what Den Lite understood.
    pub fn client_op(&mut self, op: u32) -> Option<String> {
        if let Some(n) = self.ops.get_mut(&op) {
            *n += 1;
            return None;
        }
        if self.ops.len() >= OPS_KEPT {
            return None;
        }
        self.ops.insert(op, 1);
        Some(client_op_name(op))
    }

    /// CMSG_TIME_SYNC_RESP: Some(detail) the first time (the `time_sync` event).
    pub fn synced(&mut self, body: &[u8]) -> Option<String> {
        let r = time_sync_resp(body)?;
        if self.time_sync.is_some() {
            return None;
        }
        self.time_sync = Some(r);
        Some(format!("the game answered the time sync (counter {}, its clock {} ms)", r.0, r.1))
    }

    /// What the game sent, for `.den where` and the logout's `moves` event: "SET_ACTIVE_MOVER x1, PING x2".
    pub fn ops_line(&self) -> String {
        if self.ops.is_empty() {
            return "nothing".into();
        }
        self.ops.iter().map(|(op, n)| format!("{} x{n}", client_op_name(*op))).collect::<Vec<_>>().join(", ")
    }

    /// One line for `.den where` and the logout's `moves` event.
    pub fn summary(&self) -> String {
        let mover = if self.mover { "the game took its character" } else { "the game never said it took its character" };
        let moves = match self.first {
            None => "no movement from the game".to_string(),
            Some(op) => format!("{} movement packets (first {})", self.moves, op_name(op)),
        };
        let walked = match self.walked {
            Some(d) => format!("walked ({d:.1} yards)"),
            None => "never walked".into(),
        };
        let flags = flag_names(self.flags_seen & !FALLING);
        let sync = if self.time_sync.is_some() { "time sync answered" } else { "time sync not answered" };
        format!("{mover}; {moves}; {walked}{}; {sync}", if flags.is_empty() { String::new() } else { format!("; flags seen: {flags}") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32, z: f32) -> At {
        At { x, y, z, o: 0.0 }
    }

    #[test]
    fn a_game_that_falls_lands_and_walks() {
        let spawn = at(16226.2, 16257.0, 13.2 + SPAWN_LIFT);
        let mut w = Watch::new(1, spawn, 13.2);
        assert!(w.mover(), "the first SET_ACTIVE_MOVER");
        assert!(!w.mover(), "only once");
        let n = w.moved(MSG_MOVE_FALL_LAND, 0, at(16226.2, 16257.0, 13.2), 13.2);
        assert_eq!(n[0], ("first_move", "FALL_LAND, z 13.20, on the ground (+0.00)".to_string()));
        assert_eq!(n[1], ("landed", "z 13.20, on the ground (+0.00)".to_string()));
        assert_eq!(w.on_ground(), Some(true));
        // a turn on the spot isn't walking
        assert!(w.moved(0x0DA, 0, at(16226.2, 16257.0, 13.2), 13.2).is_empty());
        assert!(w.moved(0x0B5, FORWARD, at(16226.7, 16257.0, 13.2), 13.2).is_empty(), "half a yard");
        let n = w.moved(0x0EE, FORWARD, at(16229.2, 16257.0, 13.21), 13.2);
        assert_eq!(n, vec![("walked", "3.0 yards by HEARTBEAT, z 13.21, on the ground (+0.01)".to_string())]);
        assert!(w.moved(0x0EE, FORWARD, at(16235.0, 16257.0, 13.2), 13.2).is_empty(), "walked is said once");
        assert_eq!(w.summary(), "the game took its character; 5 movement packets (first FALL_LAND); walked (3.0 yards); time sync not answered");
    }

    #[test]
    fn a_character_held_up_or_still_is_named() {
        let mut w = Watch::new(1, at(0.0, 0.0, 14.2), 13.2);
        assert_eq!(w.on_ground(), None);
        let n = w.moved(0x0EE, ROOT | DISABLE_GRAVITY | FALLING, at(0.0, 0.0, 14.2), 13.2);
        assert_eq!(n[0].1, "HEARTBEAT [ROOT DISABLE_GRAVITY FALLING], z 14.20, 1.00 above the ground");
        assert_eq!(n[1], ("move_flags", "ROOT DISABLE_GRAVITY in HEARTBEAT".to_string()));
        assert_eq!(w.on_ground(), Some(false));
        assert!(w.moved(0x0EE, ROOT, at(0.0, 0.0, 14.2), 13.2).is_empty(), "a flag is told once");
        // falling alone is every jump: not told
        let mut j = Watch::new(2, at(0.0, 0.0, 13.2), 13.2);
        assert_eq!(j.moved(0x0BB, FALLING, at(0.0, 0.0, 13.2), 13.2).len(), 1, "only the first move");
        assert_eq!(w.summary(), "the game never said it took its character; 2 movement packets (first HEARTBEAT); never walked; flags seen: ROOT DISABLE_GRAVITY; time sync not answered");
        assert_eq!(Watch::new(3, at(0.0, 0.0, 0.0), 0.0).summary(), "the game never said it took its character; no movement from the game; never walked; time sync not answered");
    }

    #[test]
    fn standing_against_the_ground() {
        assert_eq!(stand(13.25, 13.2), "on the ground (+0.05)");
        assert_eq!(stand(14.2, 13.2), "1.00 above the ground");
        assert_eq!(stand(12.0, 13.2), "1.20 under the ground");
        assert_eq!(
            spawn_note(at(1.0, 2.0, 13.25), 13.2, true, Some(3.04)),
            "z 13.25, ground 13.20 (the Den's ground file), on the ground (+0.05), nearest prop 3.0 yards"
        );
        assert!(spawn_note(at(1.0, 2.0, 13.25), 13.2, false, None).ends_with("(the fixed floor: no ground file), on the ground (+0.05), nearest prop none"));
    }

    #[test]
    fn packets_read_and_written_by_hand() {
        // a heartbeat as the game sends it: packed guid (mask 0x01, guid 1), flags, flags2, time, x y z o, fall time
        let mut b = vec![0x01, 0x01];
        b.extend_from_slice(&(ROOT | FORWARD).to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&1234u32.to_le_bytes());
        for v in [16226.2f32, 16257.0, 13.2, 1.5] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(raw_flags(&b), Some(ROOT | FORWARD));
        let p = crate::world::raw_move(0x0EE, &b).unwrap();
        assert_eq!((p.x, p.y, p.z), (16226.2, 16257.0, 13.2));
        assert_eq!(raw_flags(&[0x03, 1]), None, "too short");
        assert_eq!(raw_flags(&[]), None);
        assert_eq!(control_body(1), vec![0x01, 0x01, 1]);
        assert_eq!(control_body(0x0100_0000_0000_0002), vec![0x81, 0x02, 0x01, 1]);
        assert_eq!(unroot_body(1, 7), vec![0x01, 0x01, 7, 0, 0, 0]);
    }

    #[test]
    fn names() {
        assert_eq!(flag_names(0), "");
        assert_eq!(flag_names(HOVER | FORWARD), "HOVER");
        assert_eq!(op_name(0x0B5), "START_FORWARD");
        assert_eq!(op_name(0x0C9), "FALL_LAND");
        assert_eq!(op_name(0x0EE), "HEARTBEAT");
        assert_eq!(op_name(0x123), "MOVE");
    }

    #[test]
    fn the_players_create_carries_azerothcores_update_flags() {
        // Unit::Unit: LIVING | STATIONARY_POSITION; BuildCreateUpdateBlockForPlayer adds SELF for the player's own
        assert_eq!(player_update_flags(true), 0x0061);
        assert_eq!(player_update_flags(false), 0x0060);
    }

    #[test]
    fn the_movement_clock_is_never_zero_and_runs_forward() {
        let a = server_ms();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = server_ms();
        assert!(a >= 1000 && b >= a + 15, "{a} {b}");
    }

    #[test]
    fn time_sync_bodies_and_the_games_answer() {
        assert_eq!(time_sync_body(7), vec![7, 0, 0, 0]);
        let mut w = Watch::new(1, at(0.0, 0.0, 0.0), 0.0);
        let mut resp = 0u32.to_le_bytes().to_vec();
        resp.extend_from_slice(&123_456u32.to_le_bytes());
        assert_eq!(time_sync_resp(&resp), Some((0, 123_456)));
        assert_eq!(time_sync_resp(&resp[..7]), None);
        assert_eq!(w.synced(&resp).as_deref(), Some("the game answered the time sync (counter 0, its clock 123456 ms)"));
        assert_eq!(w.synced(&resp), None, "said once");
        assert!(w.summary().ends_with("; time sync answered"), "{}", w.summary());
    }

    #[test]
    fn every_opcode_the_game_sends_is_written_down_once() {
        let mut w = Watch::new(1, at(0.0, 0.0, 0.0), 0.0);
        assert_eq!(w.ops_line(), "nothing");
        assert_eq!(w.client_op(0x26A).as_deref(), Some("SET_ACTIVE_MOVER"));
        assert_eq!(w.client_op(0x0B5).as_deref(), Some("MSG_MOVE_START_FORWARD"));
        assert_eq!(w.client_op(0x2CE).as_deref(), Some("MOVE_TIME_SKIPPED"));
        assert_eq!(w.client_op(0x26A), None, "only the first time");
        assert_eq!(w.client_op(0x4242).as_deref(), Some("0x4242"));
        assert_eq!(w.ops_line(), "MSG_MOVE_START_FORWARD x1, SET_ACTIVE_MOVER x2, MOVE_TIME_SKIPPED x1, 0x4242 x1");
        for op in 0..100u32 {
            w.client_op(0x5000 + op);
        }
        assert_eq!(w.ops.len(), OPS_KEPT, "bounded");
    }
}
