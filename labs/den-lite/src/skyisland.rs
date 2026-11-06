//! Sky Island (owner 5 Nov; the design is the project doc claude/SKY-ISLAND-SMALLEST-SLICE.md): a floating island
//! about 60 yards across from its middle, open sky all round, one big tree in the middle; you land on it at once (the
//! launcher's one click: no login screen, no character select), with training dummies to hit, Jaina to spar beside
//! and the whole Den editor. Made of the player's OWN game's models (Den Lite names gameobject entries; nothing of the
//! game is shipped).
//!
//! The island is a Den look, not props: `room.look = "sky"` makes Den Lite lay the ground, the rim and the tree as the
//! room's own pieces ([`plan`]), the way the stone room lays its floor. So a Sky Island Den file (or a code: denpack.rs)
//! carries only what the player added, and a whole island fits in a link. It floats: the Den is moved to
//! [`spot`], high over open sea past the edge of the map the Den's home is on, where the game has no ground of its own;
//! falling off puts you back by the tree ([`rescue`]).
//!
//! Std only, so `rustc --test src/skyisland.rs` checks it anywhere.

/// The look's name (den.rs LOOKS).
pub const LOOK: &str = "sky";
/// The island's radius (yards): its ground reaches this far from the middle.
pub const RADIUS: f32 = 60.0;
/// The Den's room size it opens at (den.rs ring_half: 64, the first open-ground size), so everything on it is inside.
pub const ROOM_SIZE: f64 = 64.0;
/// Where it floats: map 1 (the Den's home map), one map tile (533 yards) further out to sea than the home's
/// tile on both axes, so the launcher's grass ground (one tile) is never under it and the game draws only sky and
/// sea; 180 yards up. TODO(owner): the live check that this spot shows open sky (else pick one with `.den here`).
pub const MAP: u32 = 1;
pub const X: f32 = 15_700.0;
pub const Y: f32 = 15_700.0;
pub const Z: f32 = 180.0;
pub const NAME: &str = "Sky Island";
/// Fallen this far under the island's top: back by the tree.
pub const FALL: f32 = 14.0;
/// Sunk this far into the ground (the game drew a piece late, or its model has no collision there): back on top.
pub const SINK: f32 = 0.8;
/// Where you land and come back to: by the tree, facing it (relative to the middle; yaw in radians).
pub const LANDING: [f32; 4] = [0.0, -9.0, 0.1, std::f32::consts::FRAC_PI_2];
/// Where the training dummies stand (east of the tree, facing the landing).
pub const DUMMIES_AT: [f32; 4] = [12.0, -4.0, 0.0, std::f32::consts::PI];

/// The island's pieces: (entry, x, y, z, yaw radians, scale), relative to the middle. Entries are gameobject
/// templates Den Lite has (objects.rs); every one of them is the player's own game's model.
pub const GROUND: u32 = 190_024; // "Platform": the stone room's floor tile, 4 yards across at size 1
pub const GROUND_SCALE: f32 = 3.0;
pub const GROUND_TILE: f32 = 4.0 * GROUND_SCALE;
pub const RIM: u32 = 19_444; // "Rock Wall": the stone room's wall
pub const UNDER: u32 = 19_458; // "Rock Monument": the rock the island floats on
pub const TREE: u32 = 183_492; // "Red Riding Hood Tree": a big old tree. TODO(owner): pick the island's tree in the Den
pub const LANTERN: u32 = 19_626; // "Human Sign Post"
pub const MAX_PIECES: usize = 160;

pub fn plan() -> Vec<(u32, f32, f32, f32, f32, f32)> {
    let mut out = Vec::new();
    // the ground: tiles on a square grid whose middles are inside the disc (a tile's half sticks out at most)
    let n = (RADIUS / GROUND_TILE).ceil() as i32;
    for i in -n..=n {
        for j in -n..=n {
            let (x, y) = (i as f32 * GROUND_TILE, j as f32 * GROUND_TILE);
            if (x * x + y * y).sqrt() <= RADIUS - GROUND_TILE * 0.35 {
                out.push((GROUND, x, y, 0.0, 0.0, GROUND_SCALE));
            }
        }
    }
    // the rim: low rock walls round the edge (a guard and the island's outline), a gap to the south (the view)
    let rim = 24;
    for k in 0..rim {
        let a = k as f32 / rim as f32 * std::f32::consts::TAU;
        if (a - 3.0 * std::f32::consts::FRAC_PI_2).abs() < 0.2 {
            continue;
        }
        let r = RADIUS - 2.0;
        out.push((RIM, r * a.cos(), r * a.sin(), 0.0, a + std::f32::consts::FRAC_PI_2, 1.0));
    }
    // under it: rocks in a ring and a bigger one in the middle, so it reads as an island floating, not a floor
    for k in 0..10 {
        let a = k as f32 / 10.0 * std::f32::consts::TAU + 0.3;
        let r = RADIUS * 0.7;
        out.push((UNDER, r * a.cos(), r * a.sin(), -9.0 - (k % 3) as f32 * 3.0, a, 3.5));
    }
    out.push((UNDER, 0.0, 0.0, -22.0, 0.0, 5.0));
    // the tree in the middle
    out.push((TREE, 0.0, 0.0, 0.0, 0.0, 2.0));
    // a sign post by the landing
    out.push((LANTERN, 3.0, -12.0, 0.0, std::f32::consts::FRAC_PI_2, 1.0));
    out
}

/// What to do with a player at `rel` (relative to the island's middle and top), moving with the game's `flying`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rescue {
    /// Fallen off: back by the tree ([`LANDING`]).
    Landing,
    /// Sunk into the ground: back on top where they are (the height to put them at, relative).
    OnTop(f32),
}

pub fn rescue(rel: [f32; 3], flying: bool) -> Option<Rescue> {
    if !rel.iter().all(|v| v.is_finite()) {
        return None;
    }
    if rel[2] < -FALL {
        return Some(Rescue::Landing);
    }
    let on = (rel[0] * rel[0] + rel[1] * rel[1]).sqrt() <= RADIUS - 1.0;
    (!flying && on && rel[2] < -SINK).then_some(Rescue::OnTop(0.1))
}

/// A string as JSON.
pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The island's first tale (a welcome that walks you round it).
pub const WELCOME_TALE: &str = "tale Welcome to Sky Island
by Jaina
about A first walk round your floating island.
sign edge at 0 -50 \"Careful: the sky is a long way down. (You'll land back by the tree.)\"
step Say hello
  say jaina \"Welcome to your island! Come here a moment.\"
  goal talk jaina
step Touch the old tree
  say jaina \"Everything here grows from that tree. Go and touch it.\"
  goal reach 0 0 7
step Spar a little
  say jaina \"The dummies by the tree can take a hit. Show me three good ones.\"
  goal defeat 3 dummies
  hint Target a dummy and attack (or cast at it).
step Look over the edge
  goal visit edge
  done jaina \"Now it's yours: press F7 to build. Make something, then pass it on.\"
reward title Sky Walker
reward cheer
";

/// The Sky Island Den file (`leyline-den` JSON): the look, the place in the sky, the welcome tale. `name`: its file
/// name. Nothing but the look and the tale: the island itself is the look's pieces.
pub fn den_json(name: &str) -> String {
    format!(
        concat!(
            "{{\"format\": \"leyline-den\", \"version\": 1, \"name\": {name}, \"title\": {title}, ",
            "\"line\": \"A floating island: build, spar, pass it on.\", ",
            "\"room\": {{\"look\": \"{look}\", \"size\": {size}}}, \"props\": [], ",
            "\"place\": {{\"map\": {map}, \"x\": {x}, \"y\": {y}, \"z\": {z}, \"name\": {title}, \"area\": 0}}, ",
            "\"island\": {{\"kind\": \"sky\", \"radius\": {r}}}, ",
            "\"tales\": [{tale}]}}"
        ),
        name = json_str(name),
        title = json_str(NAME),
        look = LOOK,
        size = ROOM_SIZE,
        map = MAP,
        x = X,
        y = Y,
        z = Z,
        r = RADIUS,
        tale = json_str(WELCOME_TALE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_island_is_small_round_and_whole() {
        let p = plan();
        assert!(p.len() <= MAX_PIECES, "{} pieces: light to send", p.len());
        let ground: Vec<_> = p.iter().filter(|x| x.0 == GROUND).collect();
        assert!(ground.len() >= 40 && ground.len() <= 90, "{}", ground.len());
        // every spot on the island within a tile's reach of a ground tile's middle (no holes to fall through)
        for k in 0..400 {
            let a = k as f32 * 0.37;
            let r = (k as f32 * 0.153) % (RADIUS - GROUND_TILE);
            let (x, y) = (r * a.cos(), r * a.sin());
            assert!(ground.iter().any(|g| (g.1 - x).abs() <= GROUND_TILE / 2.0 + 0.01 && (g.2 - y).abs() <= GROUND_TILE / 2.0 + 0.01), "a hole at {x} {y}");
        }
        // everything inside the room's reach (den.rs Spot::reach: size + 14)
        assert!(p.iter().all(|x| (x.1 * x.1 + x.2 * x.2).sqrt() <= ROOM_SIZE as f32 + 14.0));
        assert_eq!(p.iter().filter(|x| x.0 == TREE).count(), 1);
        assert!(p.iter().all(|x| x.5 > 0.0 && x.5 <= 5.0 && [x.1, x.2, x.3, x.4].iter().all(|v| v.is_finite())));
        // under the home's map tile? never: one whole tile further out on both axes
        let tile = |v: f32| ((17_066.666 - v) / 533.333).floor() as i32;
        assert!(tile(X) != tile(16_226.2) && tile(Y) != tile(16_257.0));
    }

    #[test]
    fn falling_off_and_sinking_in() {
        assert_eq!(rescue([0.0, 0.0, 0.0], false), None, "standing");
        assert_eq!(rescue([0.0, 0.0, 30.0], true), None, "flying over it");
        assert_eq!(rescue([0.0, 0.0, -0.5], false), None, "a step down is fine");
        assert_eq!(rescue([5.0, 5.0, -2.0], false), Some(Rescue::OnTop(0.1)), "sunk in");
        assert_eq!(rescue([5.0, 5.0, -2.0], true), None, "flying down through it on purpose");
        assert_eq!(rescue([70.0, 0.0, -5.0], false), None, "off the edge, still falling");
        assert_eq!(rescue([70.0, 0.0, -20.0], false), Some(Rescue::Landing));
        assert_eq!(rescue([0.0, 0.0, -200.0], true), Some(Rescue::Landing), "even flying, that far down");
        assert_eq!(rescue([f32::NAN, 0.0, 0.0], false), None);
    }

    #[test]
    fn the_den_file_is_json_with_its_tale() {
        let j = den_json("sky-island");
        assert!(j.starts_with("{\"format\": \"leyline-den\""));
        assert!(j.contains("\"look\": \"sky\"") && j.contains("\"size\": 64") && j.contains("\"island\""));
        assert!(j.contains("tale Welcome to Sky Island\\nby Jaina"));
        assert!(j.len() < 2048, "{} bytes: small enough for a link once deflated", j.len());
        assert_eq!(json_str("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
    }
}
