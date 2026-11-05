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
//!
//! Anywhere, fast (owner 4 Nov): the GM ways too (`.go <place>`, `.tele <place>`, `.go xyz x y z [map]`), `.back` (the
//! spot before the last teleport), `.den home` (your Den, from any map); [`search`] for the Teleports window and
//! `.den places <word>`; [`Landing`]: a character that falls through the world after a teleport is lifted back.
//! docs/DEN-PLACES.md has all of it.

use std::collections::HashMap;

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
    /// The Teleports window's tab (tools/den-places/gen_teleports.py CATS): city, town, start, zone, dungeon, raid,
    /// bg, arena, spot, den.
    pub cat: &'static str,
    pub words: &'static [&'static str],
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    /// AreaTable id (the character list's zone; the game finds its own zone from the map); 0 when not known.
    pub area: u32,
}

/// The named places (owner 4 Nov: "teleport anywhere from the Den, fast"): every zone of the four maps, the cities
/// and towns, the starting areas, the doors of the dungeons and raids, the battlegrounds, the arenas and some sights.
/// One list for Den Lite and the in-game Teleports window: tools/den-places/teleports.json, written into
/// `placelist.rs` (and the addon's TeleportsData.lua) by tools/den-places/gen_teleports.py. The numbers are where a
/// character stands, as AzerothCore's `game_tele` (and for the arenas the game's own start spots) have them: facts,
/// picked one by one (docs/DEN-PLACES.md says why that is fine and what to do if the owner wants none of them).
pub const PLACES: &[Place] = include!("placelist.rs");

/// The categories, in the window's order, with their titles.
pub const CATS: &[(&str, &str)] = &[
    ("city", "Cities"),
    ("town", "Towns"),
    ("start", "Starting areas"),
    ("zone", "Zones"),
    ("dungeon", "Dungeons"),
    ("raid", "Raids"),
    ("bg", "Battlegrounds"),
    ("arena", "Arenas"),
    ("spot", "Sights"),
    ("den", "Your Den"),
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

/// A place's names as typed: its name, its other words and its title ("Thunder Bluff": `thunderbluff`).
fn names_of(p: &Place) -> Vec<String> {
    std::iter::once(norm(p.name)).chain(std::iter::once(norm(p.title))).chain(p.words.iter().map(|w| norm(w))).collect()
}

/// How well `q` (normed) names a place: 4 its whole name or word, 3 the start of its name, 2 the start of a word or of
/// its title, 1 inside its title (4 letters or more typed), 0 not at all.
fn score(p: &Place, q: &str) -> u8 {
    let names = names_of(p);
    if names.iter().any(|n| n == q) {
        4
    } else if q.len() >= 2 && norm(p.name).starts_with(q) {
        3
    } else if q.len() >= 2 && names.iter().any(|n| n.starts_with(q)) {
        2
    } else if q.len() >= 4 && names.iter().any(|n| n.contains(q)) {
        1
    } else {
        0
    }
}

/// A named place: its whole name, a word or its title first, then the start of one (3 letters at least).
pub fn find(q: &str) -> Option<&'static Place> {
    let q = norm(q);
    if q.is_empty() {
        return None;
    }
    PLACES.iter().find(|p| score(p, &q) == 4).or_else(|| (q.len() >= 3).then(|| PLACES.iter().find(|p| score(p, &q) >= 2)).flatten())
}

/// The places a search names, best first (then the list's order), at most `max`. A category's id or title (`raid`,
/// `Raids`) gives that category.
pub fn search(q: &str, max: usize) -> Vec<&'static Place> {
    let q = norm(q);
    if q.is_empty() {
        return Vec::new();
    }
    if let Some((cat, _)) = CATS.iter().find(|(c, t)| norm(c) == q || norm(t) == q || format!("{}s", norm(c)) == q) {
        return PLACES.iter().filter(|p| p.cat == *cat).take(max).collect();
    }
    let mut hits: Vec<(u8, usize, &'static Place)> = PLACES.iter().enumerate().map(|(i, p)| (score(p, &q), i, p)).filter(|h| h.0 > 0).collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().take(max).map(|h| h.2).collect()
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
    parse_go_on(args, None)
}

/// `.den go` / `.go` / `.tele` with the map the player is on (for a point without a map): a place's name, `<map> <x>
/// <y> <z>`, `<x> <y> <z>` (this map), or the GM way, `xyz <x> <y> <z> [map]`.
pub fn parse_go_on(args: &[&str], here: Option<u32>) -> Result<Target, String> {
    if args.is_empty() {
        return Err(usage());
    }
    let nums = |s: &[&str]| s.iter().map(|w| w.trim_end_matches(',').parse::<f32>()).collect::<Result<Vec<f32>, _>>().ok();
    let (first, rest) = (args[0].to_lowercase(), &args[1..]);
    if first == "xyz" && (rest.len() == 3 || rest.len() == 4) {
        let map = if rest.len() == 4 { map_named(rest[3]) } else { here };
        if let (Some(v), Some(map)) = (nums(&rest[..3]), map) {
            return point(map, v[0], v[1], v[2]);
        }
        return Err(usage());
    }
    if args.len() == 4 {
        if let (Some(map), Some(v)) = (map_named(args[0]), nums(&args[1..])) {
            return point(map, v[0], v[1], v[2]);
        }
    }
    if let (3, Some(map), Some(v)) = (args.len(), here, nums(args)) {
        return point(map, v[0], v[1], v[2]);
    }
    let q = args.join(" ");
    find(&q).map(to_target).ok_or_else(|| {
        let like: Vec<&str> = search(&q, 4).iter().map(|p| p.name).collect();
        if like.is_empty() {
            format!("No place called '{q}'. {}", usage())
        } else {
            format!("No place called '{q}'. Did you mean {}? (.den places {q} lists them)", like.join(", "))
        }
    })
}

/// Where a named place sends you: a little above its spot (the game falls onto its own ground).
pub fn to_target(p: &Place) -> Target {
    Target { map: p.map, x: p.x, y: p.y, z: p.z + LIFT, o: p.o, area: (p.area != 0).then_some(p.area), title: p.title.into() }
}

/// A typed point, checked: on the world's maps' grid, a sensible height.
pub fn point(map: u32, x: f32, y: f32, z: f32) -> Result<Target, String> {
    if ![x, y, z].iter().all(|v| v.is_finite()) || x.abs() > EDGE || y.abs() > EDGE || z.abs() > 5000.0 {
        return Err(format!("That spot is off the map: x and y are -{EDGE:.0} to {EDGE:.0}, z -5000 to 5000."));
    }
    Ok(Target { map, x, y, z: z + TYPED_LIFT, o: 0.0, area: None, title: format!("map {map} at {x:.0}, {y:.0}, {z:.0}") })
}

pub fn usage() -> String {
    ".go <place> (or .den go; .den places <word> finds one), .go <map> <x> <y> <z>, .back (where you were), .den home (your Den). Maps: 0 eastern, 1 kalimdor, 530 outland, 571 northrend.".into()
}

/// A chat line at most this long (the list is cut into lines of whole names).
pub const LINE: usize = 230;

/// `.den places [word]`: without a word, one line per category (its title, then its names); with one, the places it
/// finds (or a category's). Lines of at most [`LINE`] letters.
pub fn list_lines(q: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |head: String, names: Vec<&str>| {
        let mut line = head.clone();
        for (i, n) in names.iter().enumerate() {
            if line.len() + n.len() + 2 > LINE {
                out.push(format!("{line},"));
                line = format!("  {n}");
            } else {
                line = if i == 0 { format!("{line} {n}") } else { format!("{line}, {n}") };
            }
        }
        out.push(format!("{line}."));
    };
    match q.map(str::trim).filter(|q| !q.is_empty()) {
        None => {
            for (cat, title) in CATS {
                let names: Vec<&str> = PLACES.iter().filter(|p| p.cat == *cat).map(|p| p.name).collect();
                if !names.is_empty() {
                    push(format!("{title}:"), names);
                }
            }
        }
        Some(q) => {
            let hits = search(q, 40);
            if hits.is_empty() {
                out.push(format!("No place matches '{q}'. .den places lists them all."));
            } else {
                push(format!("{q}:"), hits.iter().map(|p| p.name).collect());
            }
        }
    }
    out
}

// ---------------------------------------------------------------- back: where you were

/// One spot a player left by a teleport (`.back` goes there).
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub map: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
    pub title: String,
}

/// How many spots `.back` remembers per session.
pub const BACKS: usize = 10;

static BACK: std::sync::OnceLock<std::sync::Mutex<HashMap<u64, Vec<Spot>>>> = std::sync::OnceLock::new();

fn backs() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Spot>>> {
    BACK.get_or_init(|| std::sync::Mutex::new(HashMap::new())).lock().unwrap_or_else(|e| e.into_inner())
}

/// A teleport leaves `from` (kept for `.back`; the newest last, at most [`BACKS`]). Kept by session: a session's
/// spots go when it ends ([`forget`]). (A static rather than a `Shared` field: places.rs's own, no shared file.)
pub fn left(sess: u64, from: Spot) {
    let mut b = backs();
    let v = b.entry(sess).or_default();
    // the same spot twice in a row (a second .back) is one
    if v.last().is_some_and(|l| l.map == from.map && (l.x - from.x).hypot(l.y - from.y) < 2.0) {
        return;
    }
    v.push(from);
    if v.len() > BACKS {
        v.remove(0);
    }
}

/// `.back`: the newest spot left (taken off the list), if any.
pub fn back(sess: u64) -> Option<Spot> {
    backs().get_mut(&sess).and_then(|v| v.pop())
}

/// The newest spot left, still kept (the window's Back button says where it goes).
pub fn peek(sess: u64) -> Option<Spot> {
    backs().get(&sess).and_then(|v| v.last().cloned())
}

pub fn forget(sess: u64) {
    backs().remove(&sess);
}

// ---------------------------------------------------------------- landing: a safe arrival anywhere

/// Falling this far below the last ground the character stood on: under the world (the typed height was inside a
/// hill, or the ground there isn't the height given). Higher than any cliff the game has, so a jump off one never
/// counts.
pub const RESCUE_DROP: f32 = 400.0;
/// ... and the character is put back this far above that ground, flying (each time once more).
pub const RESCUE_LIFT: f32 = 30.0;
/// At most this many lifts after one teleport (then back where it came from).
pub const RESCUES: u8 = 3;
/// movewatch.rs FALLING and FLYING (the movement flags).
const FALLING: u32 = 0x1000;
const FLYING: u32 = 0x200_0000;

/// What a [`Landing`] says to do with a character that fell through the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rescue {
    /// Put it here (over the last ground), flying.
    Lift(f32, f32, f32),
    /// The lifts didn't hold: back where it came from (`.back`, else its Den).
    Back,
}

/// The title of a lift's trip (world.rs keeps the landing's count through it).
pub const LIFTED: &str = "a safe spot over the ground";

/// After a teleport to anywhere: Den Lite knows no ground there (the game has it, from the player's own files), so it
/// watches. Every move on the ground is the last ground; a fall far under it ([`RESCUE_DROP`]) is a fall through the
/// world, and the character is lifted back over the last ground with flying on, to come down on the game's own ground.
#[derive(Debug, Clone, PartialEq)]
pub struct Landing {
    pub map: u32,
    /// The last ground: where it arrived, then where it last stood.
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rescues: u8,
}

impl Landing {
    pub fn new(map: u32, x: f32, y: f32, z: f32) -> Landing {
        Landing { map, x, y, z, rescues: 0 }
    }

    /// A move of the character (its map, spot and movement flags): what to do when it fell through.
    pub fn check(&mut self, map: u32, x: f32, y: f32, z: f32, flags: u32) -> Option<Rescue> {
        if map != self.map || !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return None;
        }
        let falling = flags & FALLING != 0 && flags & FLYING == 0;
        if !falling {
            (self.x, self.y, self.z) = (x, y, z);
            return None;
        }
        if z >= self.z - RESCUE_DROP || self.rescues > RESCUES {
            return None;
        }
        self.rescues += 1;
        if self.rescues > RESCUES {
            return Some(Rescue::Back);
        }
        // higher each time: 30, 120, 270 yards over the last ground (a typed height deep inside a mountain)
        let n = self.rescues as f32;
        Some(Rescue::Lift(self.x, self.y, self.z + RESCUE_LIFT * n * n))
    }
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
            assert!(p.x.abs() < EDGE && p.y.abs() < EDGE && p.map < 1000, "{}", p.name);
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
        assert!(list_lines(None).iter().any(|l| l.starts_with("Cities: stormwind")));
    }

    #[test]
    fn the_list_is_whole_and_sane() {
        // every category has places; the four maps' zones are there; dungeons, raids, arenas
        for (cat, _) in CATS {
            assert!(PLACES.iter().any(|p| p.cat == *cat), "{cat}");
        }
        assert!(PLACES.len() >= 200, "{}", PLACES.len());
        for m in [0, 1, 530, 571] {
            assert!(PLACES.iter().filter(|p| p.cat == "zone" && p.map == m).count() >= 8, "zones of map {m}");
        }
        for a in ["nagrandarena", "bladesedgearena", "ruinsoflordaeron", "dalaransewers", "ringofvalor"] {
            let p = find(a).unwrap();
            assert_eq!(p.cat, "arena");
            assert!(p.map > 571, "an arena is its own map");
        }
        for p in PLACES {
            assert!(p.z.abs() < 5000.0 && p.o.is_finite() && (0.0..=6.3).contains(&p.o), "{}", p.name);
            assert!(CATS.iter().any(|c| c.0 == p.cat), "{}", p.name);
            // a place finds itself by its name and by its title
            assert_eq!(find(p.name).unwrap().name, p.name);
            assert_eq!(find(p.title).map(|q| q.name), Some(p.name), "{}", p.title);
            let t = p.title.to_lowercase();
            for w in ["warcraft", "blizzard", "lich king"] {
                assert!(!t.contains(w), "{}", p.title);
            }
        }
        // the generated table and the addon's are the same list (tools/den-places/gen_teleports.py --check runs in its
        // own tests; here: the addon file names every place)
        let lua = include_str!("../../../stream/fui-web/Interface/AddOns/WrathUnleashedUI/TeleportsData.lua");
        for p in PLACES {
            assert!(lua.contains(&format!("{{ \"{}\", ", p.name)), "{}", p.name);
        }
    }

    #[test]
    fn search_ranks_names_then_starts_then_insides() {
        let names = |q: &str| search(q, 10).iter().map(|p| p.name).collect::<Vec<_>>();
        assert_eq!(names("stormwind")[0], "stormwind");
        assert!(names("stormwind").contains(&"stormwindharbor"));
        assert_eq!(names("icc"), vec!["icecrowncitadel"]);
        assert!(names("raids").len() == 10 && search("raid", 99).iter().all(|p| p.cat == "raid"));
        assert!(names("arena").contains(&"ringofvalor"), "a category by its id");
        assert!(names("sorrows").contains(&"swampofsorrows"), "inside a title");
        assert!(names("zz").is_empty() && names("").is_empty());
        assert_eq!(find("Un'Goro Crater").unwrap().name, "ungoro");
        assert_eq!(find("thunder bluff").unwrap().name, "thunderbluff");
        let l = list_lines(Some("plague"));
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("westernplaguelands") && l[0].contains("easternplaguelands"));
        assert!(list_lines(Some("qqqq"))[0].starts_with("No place matches"));
        for l in list_lines(None) {
            assert!(l.len() <= LINE + 2, "{l}");
        }
        assert!(list_lines(None).iter().any(|l| l.starts_with("Zones:")));
    }

    #[test]
    fn the_gm_ways_to_go() {
        let t = parse_go_on(&["xyz", "1", "2", "3"], Some(571)).unwrap();
        assert_eq!((t.map, t.x, t.y, t.z), (571, 1.0, 2.0, 3.0 + TYPED_LIFT));
        assert_eq!(parse_go_on(&["xyz", "1", "2", "3", "530"], Some(0)).unwrap().map, 530);
        assert_eq!(parse_go_on(&["-8833.4,", "628.6,", "94"], Some(0)).unwrap().map, 0, "commas as copied");
        assert!(parse_go_on(&["1", "2", "3"], None).is_err(), "no map known: not a point");
        assert!(parse_go_on(&["xyz", "1", "2"], Some(0)).is_err());
        let e = parse_go(&["stormw", "harbour"]).unwrap_err();
        assert!(e.starts_with("No place called"), "{e}");
        let e = parse_go(&["blackrok"]).unwrap_err();
        assert!(e.contains("Did you mean") || e.contains("No place"), "{e}");
        assert_eq!(parse_go(&["dalaran", "sewers"]).unwrap().map, 617);
        assert_eq!(parse_go(&["goldshire"]).unwrap().area, Some(87));
        assert_eq!(parse_go(&["lakeshire"]).unwrap().area, None, "no area id known: the game finds its zone");
    }

    #[test]
    fn back_remembers_the_spots_left() {
        let s = |x: f32| Spot { map: 0, x, y: 0.0, z: 0.0, o: 0.0, title: format!("at {x}") };
        let sess = 0xBAC0;
        assert!(back(sess).is_none());
        left(sess, s(1.0));
        left(sess, s(1.5)); // the same spot again: one
        left(sess, s(100.0));
        assert_eq!(peek(sess).unwrap().x, 100.0);
        assert_eq!(back(sess).unwrap().x, 100.0);
        assert_eq!(back(sess).unwrap().x, 1.0);
        assert!(back(sess).is_none());
        for i in 0..(BACKS + 5) {
            left(sess, s(i as f32 * 10.0));
        }
        assert_eq!(backs().get(&sess).unwrap().len(), BACKS);
        forget(sess);
        assert!(peek(sess).is_none());
    }

    #[test]
    fn a_fall_through_the_world_is_lifted_back() {
        let mut l = Landing::new(0, 10.0, 20.0, 50.0);
        // arrives a little high and falls onto the ground: nothing
        assert_eq!(l.check(0, 10.0, 20.0, 49.0, FALLING), None);
        assert_eq!(l.check(0, 11.0, 20.0, 48.0, 0), None);
        assert_eq!((l.x, l.z), (11.0, 48.0), "the last ground");
        // a jump off a high cliff lands: nothing
        assert_eq!(l.check(0, 30.0, 20.0, -200.0, FALLING), None);
        assert_eq!(l.check(0, 30.0, 20.0, -210.0, 0), None);
        // under the world: lifted over the last ground, higher each time, three times at most
        let deep = -210.0 - RESCUE_DROP - 1.0;
        assert_eq!(l.check(0, 30.0, 20.0, deep, FALLING), Some(Rescue::Lift(30.0, 20.0, -210.0 + RESCUE_LIFT)));
        assert_eq!(l.check(0, 30.0, 20.0, deep - 50.0, FALLING), Some(Rescue::Lift(30.0, 20.0, -210.0 + 4.0 * RESCUE_LIFT)));
        assert_eq!(l.check(0, 30.0, 20.0, deep, FALLING), Some(Rescue::Lift(30.0, 20.0, -210.0 + 9.0 * RESCUE_LIFT)));
        assert_eq!(l.check(0, 30.0, 20.0, deep, FALLING), Some(Rescue::Back), "then back where it came from");
        assert_eq!(l.check(0, 30.0, 20.0, deep, FALLING), None, "once");
        // flying down isn't falling; another map's moves aren't this landing's
        let mut l = Landing::new(0, 0.0, 0.0, 0.0);
        assert_eq!(l.check(0, 0.0, 0.0, -900.0, FALLING | FLYING), None);
        assert_eq!(l.check(1, 0.0, 0.0, -900.0, FALLING), None);
        assert_eq!(l.check(0, 0.0, 0.0, f32::NAN, FALLING), None);
    }
}
