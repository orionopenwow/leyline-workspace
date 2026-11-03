//! Den anywhere (owner 3 Nov: "make dens able to have whole cities or even teleport anywhere"): `.den go <place>`,
//! `.den go <map> <x> <y> <z>`, `.den here` (make my Den here) and `.den places`.
//!
//! The player's own game already has every map; Den Lite only says where to stand. Nothing here is game data: the
//! named places are coordinates (map, x, y, z, facing) and an area id, the numbers AzerothCore's `game_tele` table
//! uses, so the game draws each place from its own files. Den Lite has no collision: the height given is a little
//! above the ground ([`LIFT`]) and the game's own fall-and-land (`MSG_MOVE_FALL_LAND`) tells Den Lite where the ground
//! is.
//!
//! The 3.3.5a teleports, as AzerothCore's `Player::TeleportTo`:
//! * **near** (same map): `MSG_MOVE_TELEPORT_ACK` from the server (packed guid, a counter, the movement info at the new
//!   spot); the game moves its character and answers `MSG_MOVE_TELEPORT_ACK` (packed guid, the counter, its time);
//! * **far** (another map): `SMSG_TRANSFER_PENDING` (the map), then `SMSG_NEW_WORLD` (map, x, y, z, facing); the game
//!   shows its loading screen, drops every object it knew and answers `MSG_MOVE_WORLDPORT_ACK` (no body). Then the
//!   player's own create and everything else is sent again (world.rs `arrive_far`).
//!
//! A Den belongs to a place: its `spot` (den.rs) is the map and the middle; props are kept relative to it. `.den here`
//! keeps the Den that was there in `<dens>/places/<key>.den.json` and opens the one kept for the new spot, if any.

/// The server's MSG_MOVE_TELEPORT_ACK (the client answers with the same opcode).
pub const MSG_MOVE_TELEPORT_ACK: u16 = 0x0C7;
/// The game's answer once the new map is loaded (no body).
pub const MSG_MOVE_WORLDPORT_ACK: u16 = 0x0DC;
pub const SMSG_NEW_WORLD: u16 = 0x03E;
pub const SMSG_TRANSFER_PENDING: u16 = 0x03F;

/// How far above a named place's height the player is put (the game then falls onto its own ground).
pub const LIFT: f32 = 2.5;
/// ... and above a height the player typed.
pub const TYPED_LIFT: f32 = 0.5;
/// The folder (in the Dens folder) of the Dens kept per place.
pub const DIR: &str = "places";
/// The world's edge (a map is 64 x 64 tiles of 533.33 yards around 0).
pub const EDGE: f32 = 17066.0;

/// The maps a player can be sent to by name.
pub const MAPS: &[(u32, &[&str])] = &[
    (0, &["eastern", "eastern-kingdoms", "ek", "azeroth"]),
    (1, &["kalimdor", "kal"]),
    (530, &["outland", "expansion01"]),
    (571, &["northrend", "nr"]),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub name: &'static str,
    pub title: &'static str,
    pub words: &'static [&'static str],
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    /// AreaTable id (the character list's zone; the game finds its own zone from the map).
    pub area: u32,
}

macro_rules! place {
    ($name:expr, $title:expr, [$($w:expr),*], $map:expr, $x:expr, $y:expr, $z:expr, $o:expr, $area:expr) => {
        Place { name: $name, title: $title, words: &[$($w),*], map: $map, x: $x, y: $y, z: $z, o: $o, area: $area }
    };
}

/// The named places: capitals, starting towns and a few more (game_tele's numbers).
pub const PLACES: &[Place] = &[
    place!("den", "your first Den (the island)", ["home", "island", "gmisland"], 1, 16226.2, 16257.0, 13.2, 1.5708, 876),
    place!("stormwind", "Stormwind", ["sw", "stormwind-city"], 0, -8833.38, 628.628, 94.0066, 1.06535, 1519),
    place!("goldshire", "Goldshire", ["lionspride"], 0, -9464.0, 62.0, 56.0, 0.0, 87),
    place!("northshire", "Northshire", ["northshire-abbey"], 0, -8913.23, -137.43, 80.53, 0.0, 9),
    place!("ironforge", "Ironforge", ["if"], 0, -4918.88, -940.406, 501.564, 5.42347, 1537),
    place!("coldridge", "Coldridge Valley", ["coldridge-valley"], 0, -6240.32, 331.033, 382.758, 6.17716, 132),
    place!("undercity", "Undercity", ["uc"], 0, 1584.07, 241.987, -52.1534, 0.049647, 1497),
    place!("deathknell", "Deathknell", [], 0, 1676.71, 1678.31, 121.67, 2.70526, 154),
    place!("bootybay", "Booty Bay", ["booty", "booty-bay"], 0, -14297.2, 530.993, 8.77916, 3.98863, 35),
    place!("menethil", "Menethil Harbor", ["menethil-harbor"], 0, -3740.29, -755.08, 10.9643, 0.0, 150),
    place!("darnassus", "Darnassus", ["darn"], 1, 9949.56, 2284.21, 1341.4, 1.59587, 1657),
    place!("shadowglen", "Shadowglen", [], 1, 10311.3, 832.463, 1326.41, 5.69632, 188),
    place!("orgrimmar", "Orgrimmar", ["org", "orgr"], 1, 1629.36, -4373.39, 31.2564, 3.54839, 1637),
    place!("razorhill", "Razor Hill", ["razor-hill"], 1, 315.72, -4743.4, 10.4867, 0.0, 362),
    place!("valleyoftrials", "Valley of Trials", ["valley", "trials", "valley-of-trials"], 1, -618.518, -4251.67, 38.718, 0.0, 363),
    place!("thunderbluff", "Thunder Bluff", ["tb", "thunder-bluff"], 1, -1277.37, 124.804, 131.287, 5.22274, 1638),
    place!("narache", "Camp Narache", ["camp-narache"], 1, -2917.58, -257.98, 52.9968, 0.0, 222),
    place!("gadgetzan", "Gadgetzan", [], 1, -7177.15, -3785.34, 8.36981, 6.10237, 976),
    place!("ratchet", "Ratchet", [], 1, -956.664, -3754.71, 5.33239, 0.996637, 392),
    place!("exodar", "The Exodar", ["the-exodar"], 530, -3965.7, -11653.6, -138.844, 0.852154, 3557),
    place!("ammenvale", "Ammen Vale", ["ammen-vale"], 530, -3961.64, -13931.2, 100.615, 2.08364, 3526),
    place!("silvermoon", "Silvermoon City", ["smc", "silvermoon-city"], 530, 9487.69, -7279.2, 14.2866, 6.16478, 3487),
    place!("sunstrider", "Sunstrider Isle", ["sunstrider-isle"], 530, 10349.6, -6357.29, 33.4026, 5.31605, 3431),
    place!("shattrath", "Shattrath City", ["shatt", "shattrath-city"], 530, -1838.16, 5301.79, -12.428, 5.9517, 3703),
    place!("dalaran", "Dalaran", ["dal"], 571, 5804.15, 624.771, 647.767, 1.64, 4395),
];

/// Where `.den go` sends you.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub map: u32,
    pub x: f32,
    pub y: f32,
    /// Where the player is put (the place's height plus the lift).
    pub z: f32,
    pub o: f32,
    pub area: Option<u32>,
    pub title: String,
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// A named place: its whole name or one of its words first, then the start of one.
pub fn find(q: &str) -> Option<&'static Place> {
    let q = norm(q);
    if q.is_empty() {
        return None;
    }
    let names = |p: &Place| std::iter::once(norm(p.name)).chain(p.words.iter().map(|w| norm(w))).collect::<Vec<_>>();
    PLACES.iter().find(|p| names(p).contains(&q)).or_else(|| (q.len() >= 3).then(|| PLACES.iter().find(|p| names(p).iter().any(|n| n.starts_with(&q)))).flatten())
}

/// A map by number or name.
pub fn map_named(w: &str) -> Option<u32> {
    let w = w.trim().to_lowercase();
    if let Ok(n) = w.parse::<u32>() {
        return MAPS.iter().any(|m| m.0 == n).then_some(n).or(Some(n).filter(|n| *n < 1000));
    }
    MAPS.iter().find(|m| m.1.contains(&w.as_str())).map(|m| m.0)
}

/// `.den go` 's words (after `go`): a place's name (several words joined), or `<map> <x> <y> <z>`.
pub fn parse_go(args: &[&str]) -> Result<Target, String> {
    if args.is_empty() {
        return Err(usage());
    }
    if args.len() == 4 {
        if let (Some(map), Ok(x), Ok(y), Ok(z)) = (map_named(args[0]), args[1].parse::<f32>(), args[2].parse::<f32>(), args[3].parse::<f32>()) {
            return point(map, x, y, z);
        }
    }
    let q = args.join(" ");
    find(&q).map(|p| Target { map: p.map, x: p.x, y: p.y, z: p.z + LIFT, o: p.o, area: Some(p.area), title: p.title.into() })
        .ok_or_else(|| format!("No place called '{q}'. {}", usage()))
}

/// A typed point, checked: on the world's maps' grid, a sensible height.
pub fn point(map: u32, x: f32, y: f32, z: f32) -> Result<Target, String> {
    if ![x, y, z].iter().all(|v| v.is_finite()) || x.abs() > EDGE || y.abs() > EDGE || z.abs() > 5000.0 {
        return Err(format!("That spot is off the map: x and y are -{EDGE:.0} to {EDGE:.0}, z -5000 to 5000."));
    }
    Ok(Target { map, x, y, z: z + TYPED_LIFT, o: 0.0, area: None, title: format!("map {map} at {x:.0}, {y:.0}, {z:.0}") })
}

pub fn usage() -> String {
    ".den go <place> (.den places lists them) or .den go <map> <x> <y> <z> (maps: 0 eastern, 1 kalimdor, 530 outland, 571 northrend).".into()
}

/// The places, in one line per map.
pub fn list_lines() -> Vec<String> {
    let mut out = Vec::new();
    for (map, words) in MAPS {
        let names: Vec<&str> = PLACES.iter().filter(|p| p.map == *map).map(|p| p.name).collect();
        if !names.is_empty() {
            out.push(format!("{}: {}.", words[0].replace('-', " "), names.join(", ")));
        }
    }
    out
}

/// The file name (without `.den.json`) of the Den kept for a spot: `m<map>_<x>_<y>`, whole yards (letters, digits,
/// - and _: den::clean_name takes it).
pub fn key(map: u32, x: f32, y: f32) -> String {
    format!("m{map}_{}_{}", x.round() as i64, y.round() as i64).replace('-', "n")
}

/// The named place nearest a spot (within 400 yards on the same map), for a Den's name.
pub fn near(map: u32, x: f32, y: f32) -> Option<&'static Place> {
    PLACES
        .iter()
        .filter(|p| p.map == map)
        .map(|p| (p, (p.x - x).hypot(p.y - y)))
        .filter(|(_, d)| *d <= 400.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(p, _)| p)
}

/// Whether going from `from` to `to` is a near teleport (the same map) or a far one (a loading screen).
pub fn is_near(from: u32, to: u32) -> bool {
    from == to
}

fn packed_guid(out: &mut Vec<u8>, guid: u64) {
    let mut mask = 0u8;
    let mut bytes = Vec::new();
    for i in 0..8 {
        let b = (guid >> (i * 8)) as u8;
        if b != 0 {
            mask |= 1 << i;
            bytes.push(b);
        }
    }
    out.push(mask);
    out.extend(bytes);
}

/// The server's MSG_MOVE_TELEPORT_ACK: packed guid, the counter, then the movement info (flags 0, flags2 0, the time,
/// x, y, z, facing, fall time 0), as AzerothCore's `Player::SendTeleportPacket` with `Unit::BuildMovementPacket`.
pub fn teleport_body(guid: u64, counter: u32, time: u32, x: f32, y: f32, z: f32, o: f32) -> Vec<u8> {
    let mut b = Vec::with_capacity(40);
    packed_guid(&mut b, guid);
    b.extend_from_slice(&counter.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&time.to_le_bytes());
    for v in [x, y, z, o] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&0u32.to_le_bytes());
    b
}

/// The game's MSG_MOVE_TELEPORT_ACK: (guid, counter).
pub fn teleport_ack(body: &[u8]) -> Option<(u64, u32)> {
    let mask = *body.first()?;
    let mut guid = 0u64;
    let mut at = 1;
    for i in 0..8 {
        if mask & (1 << i) != 0 {
            guid |= (*body.get(at)? as u64) << (i * 8);
            at += 1;
        }
    }
    let c = body.get(at..at + 4)?;
    Some((guid, u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
}

/// SMSG_TRANSFER_PENDING: the map (no transport).
pub fn transfer_pending_body(map: u32) -> Vec<u8> {
    map.to_le_bytes().to_vec()
}

/// SMSG_NEW_WORLD: map, x, y, z, facing.
pub fn new_world_body(map: u32, x: f32, y: f32, z: f32, o: f32) -> Vec<u8> {
    let mut b = map.to_le_bytes().to_vec();
    for v in [x, y, z, o] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// A trip that was sent and waits for the game's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Trip {
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    pub far: bool,
    pub counter: u32,
    pub area: Option<u32>,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_by_name_word_or_start() {
        assert_eq!(find("Stormwind").unwrap().map, 0);
        assert_eq!(find("org").unwrap().name, "orgrimmar");
        assert_eq!(find("Thunder Bluff").unwrap().name, "thunderbluff");
        assert_eq!(find("dala").unwrap().map, 571);
        assert_eq!(find("home").unwrap().name, "den");
        assert!(find("zz").is_none() && find("").is_none());
        // every place is on the map's grid and has a unique name
        for p in PLACES {
            assert!(p.x.abs() < EDGE && p.y.abs() < EDGE && MAPS.iter().any(|m| m.0 == p.map), "{}", p.name);
            assert_eq!(PLACES.iter().filter(|q| q.name == p.name).count(), 1);
            assert!(crate::den::clean_name(&key(p.map, p.x, p.y)).is_some(), "{}", key(p.map, p.x, p.y));
        }
    }

    #[test]
    fn go_takes_a_place_or_a_point() {
        let t = parse_go(&["goldshire"]).unwrap();
        assert_eq!((t.map, t.x, t.y), (0, -9464.0, 62.0));
        assert_eq!(t.z, 56.0 + LIFT, "a little above: the game falls onto its own ground");
        let t = parse_go(&["valley", "of", "trials"]).unwrap();
        assert_eq!(t.title, "Valley of Trials");
        let t = parse_go(&["northrend", "5804", "624.7", "647"]).unwrap();
        assert_eq!((t.map, t.z), (571, 647.0 + TYPED_LIFT));
        assert_eq!(parse_go(&["0", "1", "2", "3"]).unwrap().map, 0);
        assert!(parse_go(&["0", "99999", "2", "3"]).unwrap_err().contains("off the map"));
        assert!(parse_go(&[]).is_err());
        assert!(parse_go(&["nowhere"]).unwrap_err().starts_with("No place called 'nowhere'"));
        assert_eq!(map_named("outland"), Some(530));
        assert_eq!(key(0, -8833.4, 628.6), "m0_n8833_629");
    }

    #[test]
    fn the_teleport_bodies() {
        let b = teleport_body(1, 7, 1000, 1.0, 2.0, 3.0, 0.5);
        assert_eq!(&b[..2], &[0x01, 0x01], "packed guid 1");
        assert_eq!(&b[2..6], &7u32.to_le_bytes());
        assert_eq!(&b[6..12], &[0; 6], "no flags");
        assert_eq!(&b[12..16], &1000u32.to_le_bytes());
        assert_eq!(f32::from_le_bytes([b[20], b[21], b[22], b[23]]), 2.0);
        assert_eq!(b.len(), 2 + 4 + 4 + 2 + 4 + 16 + 4);
        assert_eq!(teleport_ack(&[0x01, 0x01, 7, 0, 0, 0, 0xE8, 3, 0, 0]), Some((1, 7)));
        assert_eq!(teleport_ack(&[0x01]), None);
        assert_eq!(new_world_body(571, 1.0, 2.0, 3.0, 4.0).len(), 20);
        assert_eq!(transfer_pending_body(530), 530u32.to_le_bytes().to_vec());
        assert!(is_near(0, 0) && !is_near(0, 1));
        assert_eq!(near(0, -9460.0, 70.0).unwrap().name, "goldshire");
        assert!(near(0, 0.0, 0.0).is_none());
        assert!(list_lines().iter().any(|l| l.starts_with("northrend: dalaran")));
    }
}
