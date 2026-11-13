//! Boss Rush (docs/BOSS-RUSH.md; owner 5 Nov: "isolate bossfights into its own launched mini game mode ... can select
//! any fight any raid light weight", then "in the actual raid ... exactly as-is", then "1:1 to the game"): pick a
//! fight, a difficulty and a hero, one click, and the game opens on that fight's own platform.
//!
//! Two engines behind the one tile:
//! * **Exact** (the main one): the real fight, played by the core's own boss script, on the player's own PC: the
//!   Built-in engine (host/engine.rs `Kind::BossRush`, its own small install beside Single player's: the same engine
//!   files, only the raid maps read from the game folder) with mod-leyline-bossfights and its heroes (an instant
//!   level 80 on the linked account `<ACCOUNT>BF`, straight onto the platform, bot companions in the other seats).
//! * **Light** (labelled "light version, not exact"): Den Lite's own recreation at the same real spot
//!   (labs/den-lite `bossrush.rs`): starts in seconds, no database; the mechanics are ours, not the game's.
//!
//! Everything here is std only and pure (the catalog, the pick, the console lines and their answers, the board of
//! best times, the share code), so `rustc --edition 2021 --test src/host/bossrush.rs` checks it anywhere; app.rs
//! does the I/O. The catalog mirrors the module's `BossfightsLogic.h` (a test reads that file and compares).

use std::fmt::Write as _;

/// One fight of the catalog (BossfightsLogic.h Bosses(), the same order: the featured one first).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fight {
    pub key: &'static str,
    pub title: &'static str,
    /// The raid's own place name (shown under the title).
    pub raid: &'static str,
    pub map: u32,
    pub heroic: bool,
    /// Den Lite has a light version of it (labs/den-lite bossrush.rs CATALOG).
    pub light: bool,
    /// What the tile says about it (TODO(owner) where his words are wanted).
    pub blurb: &'static str,
}

pub const FIGHTS: &[Fight] = &[
    Fight { key: "frozen-throne", title: "The Frozen Throne", raid: "Icecrown Citadel", map: 631, heroic: true, light: true,
        blurb: "The citadel's last fight, on the platform at its top: adds, the edge falling away, the other realm inside the blade." },
    Fight { key: "bone-lord", title: "The Bone Lord", raid: "Icecrown Citadel", map: 631, heroic: true, light: true,
        blurb: "The first hall: spikes that pin a friend, a whirl to run from, fire on the floor." },
    Fight { key: "cult-mistress", title: "The Cult Mistress", raid: "Icecrown Citadel", map: 631, heroic: true, light: true,
        blurb: "A mana shield to break while the cult pours in from both doors." },
    Fight { key: "twilight-destroyer", title: "The Twilight Destroyer", raid: "The Ruby Sanctum", map: 724, heroic: true, light: true,
        blurb: "One dragon in two realms: split the raid, keep both halves even." },
    Fight { key: "obsidian-sanctum", title: "The Obsidian Sanctum", raid: "The Obsidian Sanctum", map: 615, heroic: false, light: true,
        blurb: "A dragon on its island in a lake of fire; walls of flame sweep the room." },
    Fight { key: "obsidian-three", title: "The Obsidian Sanctum, three drakes", raid: "The Obsidian Sanctum", map: 615, heroic: false, light: false,
        blurb: "The same fight with its three drakes still up: they join one by one." },
    Fight { key: "brood-mother", title: "The Brood Mother's Lair", raid: "Onyxia's Lair", map: 249, heroic: false, light: true,
        blurb: "A dragon in her cave: she takes to the air, whelps pour out, then she lands for the end." },
    Fight { key: "stone-watcher", title: "The Stone Watcher", raid: "Vault of Archavon", map: 624, heroic: false, light: true,
        blurb: "A stone giant in the vault: a quick fight to learn the ropes." },
    Fight { key: "storm-watcher", title: "The Storm Watcher", raid: "Vault of Archavon", map: 624, heroic: false, light: false,
        blurb: "Lightning and its minions: one of them overcharges, kill it before it bursts." },
    Fight { key: "flame-watcher", title: "The Flame Watcher", raid: "Vault of Archavon", map: 624, heroic: false, light: false,
        blurb: "Burning fists and a cross of fire across the room." },
    Fight { key: "frost-watcher", title: "The Frost Watcher", raid: "Vault of Archavon", map: 624, heroic: false, light: false,
        blurb: "Frost orbs drift through the room; the cold stacks up." },
    Fight { key: "spell-weaver", title: "The Spellweaver's Eye", raid: "The Eye of Eternity", map: 616, heroic: false, light: false,
        blurb: "A disc in the sky: sparks, a vortex, and at the end the floor is gone and you fight on dragons." },
    // the rest of the citadel (start anywhere, owner 12 Nov; BossfightsLogic.h, the same order). TODO(owner): his words
    // for each line.
    Fight { key: "icc-gunship", title: "The Gunship Battle", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Two ships in the air: cannons, boarders, and a jump across to the other deck." },
    Fight { key: "icc-deathbringer", title: "The Deathbringer", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "A champion who grows stronger with every hit: marks, beasts, and a race against his power." },
    Fight { key: "icc-bloat", title: "The Bloated Horror", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Gas fills the room and spores spread it: stack up, spread out, keep breathing." },
    Fight { key: "icc-ooze", title: "The Ooze Horror", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Oozes that merge and burst: kite them round the room while the floor sprays." },
    Fight { key: "icc-professor", title: "The Mad Professor", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "The lab's master and his experiments: puddles, gas clouds, and a potion that changes everything." },
    Fight { key: "icc-council", title: "The Blood Council", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Three princes, one of them empowered at a time: switch to the right one." },
    Fight { key: "icc-blood-queen", title: "The Blood Queen", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Bites that spread a thirst through the raid, and swarms in the air above her hall." },
    Fight { key: "icc-dreamwalker", title: "The Dreamwalker", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "Heal a dreaming dragon back to full while the adds pour in from every gate." },
    Fight { key: "icc-frost-queen", title: "The Frost Queen", raid: "Icecrown Citadel", map: 631, heroic: true, light: false,
        blurb: "A frost dragon over her courtyard: ice tombs, a pull to her, and the cold that stacks up." },
];

// ------------------------------------------------------------------------------------------------ start anywhere

/// Boss Rush's "start anywhere" (owner 12 Nov: "start in front of any boss or any raid wing or the entire raid itself
/// with companions or with friends in a one-click UI"): the module's tree (BossfightsStarts.h Raids(), the same order),
/// as the picker shows it. A wing of one boss has no key of its own (it plays the boss).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wing {
    pub key: &'static str,
    pub title: &'static str,
    /// Its bosses' fights in order (the steps without a start of their own, the gauntlet, are left out).
    pub fights: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Raid {
    pub key: &'static str,
    pub title: &'static str,
    pub map: u32,
    pub heroic: bool,
    pub fights: &'static [&'static str],
    pub wings: &'static [Wing],
    /// The whole raid from its entrance ("" = none: a raid of one fight).
    pub whole: &'static str,
}

pub const RAIDS: &[Raid] = &[
    Raid { key: "icc", title: "Icecrown Citadel", map: 631, heroic: true,
        fights: &["bone-lord", "cult-mistress", "icc-gunship", "icc-deathbringer", "icc-bloat", "icc-ooze", "icc-professor", "icc-council", "icc-blood-queen", "icc-dreamwalker", "icc-frost-queen", "frozen-throne"],
        wings: &[
            Wing { key: "icc-spire", title: "The Lower Spire", fights: &["bone-lord", "cult-mistress", "icc-deathbringer"] },
            Wing { key: "icc-plague", title: "The Plagueworks", fights: &["icc-bloat", "icc-ooze", "icc-professor"] },
            Wing { key: "icc-crimson", title: "The Crimson Hall", fights: &["icc-council", "icc-blood-queen"] },
            Wing { key: "icc-frostwing", title: "The Frostwing Halls", fights: &["icc-dreamwalker", "icc-frost-queen"] },
            Wing { key: "", title: "The Frozen Throne", fights: &["frozen-throne"] },
        ],
        whole: "icc-raid" },
    Raid { key: "rs", title: "The Ruby Sanctum", map: 724, heroic: true, fights: &["twilight-destroyer"],
        wings: &[Wing { key: "", title: "The Ruby Sanctum", fights: &["twilight-destroyer"] }], whole: "rs-raid" },
    Raid { key: "os", title: "The Obsidian Sanctum", map: 615, heroic: false, fights: &["obsidian-sanctum", "obsidian-three"],
        wings: &[Wing { key: "", title: "The Obsidian Sanctum", fights: &["obsidian-sanctum"] }], whole: "" },
    Raid { key: "ony", title: "Onyxia's Lair", map: 249, heroic: false, fights: &["brood-mother"],
        wings: &[Wing { key: "", title: "Onyxia's Lair", fights: &["brood-mother"] }], whole: "" },
    Raid { key: "voa", title: "Vault of Archavon", map: 624, heroic: false, fights: &["stone-watcher", "storm-watcher", "flame-watcher", "frost-watcher"],
        wings: &[Wing { key: "", title: "Vault of Archavon", fights: &["stone-watcher"] }], whole: "voa-raid" },
    Raid { key: "eoe", title: "The Eye of Eternity", map: 616, heroic: false, fights: &["spell-weaver"],
        wings: &[Wing { key: "", title: "The Eye of Eternity", fights: &["spell-weaver"] }], whole: "" },
];

/// What a start is: one boss, a wing, or the whole raid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Boss,
    Wing,
    Raid,
}

impl Scope {
    pub fn word(self) -> &'static str {
        match self {
            Scope::Boss => "boss",
            Scope::Wing => "wing",
            Scope::Raid => "raid",
        }
    }
}

/// A start by its key: (scope, its title, its raid).
pub fn start(key: &str) -> Option<(Scope, &'static str, &'static Raid)> {
    for r in RAIDS {
        if r.fights.contains(&key) {
            return fight(key).map(|f| (Scope::Boss, f.title, r));
        }
        if let Some(w) = r.wings.iter().find(|w| !w.key.is_empty() && w.key == key) {
            return Some((Scope::Wing, w.title, r));
        }
        if !r.whole.is_empty() && r.whole == key {
            return Some((Scope::Raid, r.title, r));
        }
    }
    None
}

/// Who comes along: companions only, or friends (and companions in the empty seats, unless `companions` is off).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    Companions,
    Friends { companions: bool },
}

impl Party {
    /// The module's mode word (docs/BOSS-FIGHTS.md): bots now, a lobby friends join, or a lobby of players only.
    pub fn mode(self) -> &'static str {
        match self {
            Party::Companions => "bots",
            Party::Friends { companions: true } => "group",
            Party::Friends { companions: false } => "group-only",
        }
    }
    pub fn of(w: &str, companions: bool) -> Option<Party> {
        match w {
            "companions" | "" => Some(Party::Companions),
            "friends" => Some(Party::Friends { companions }),
            _ => None,
        }
    }
}

/// The picker's one click: where, how hard, how many, who, and the hero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPick {
    pub start: String,
    pub scope: Scope,
    pub heroic: bool,
    pub size: u8,
    pub party: Party,
    pub class: String,
    pub spec: String,
}

impl StartPick {
    /// The module's difficulty word (BossfightsLogic.h DiffToken): normal, heroic, normal25, heroic25.
    pub fn diff_token(&self) -> String {
        format!("{}{}", if self.heroic { "heroic" } else { "normal" }, if self.size == 25 { "25" } else { "" })
    }
}

/// A start checked: a known key, heroic only where the raid has one, 10 or 25, a class and one of its specs.
pub fn check_start(key: &str, diff: &str, size: u8, party: &str, companions: bool, class: &str, spec: &str) -> Result<StartPick, String> {
    let (scope, title, raid) = start(key).ok_or_else(|| "Pick a boss, a wing or a whole raid from the list.".to_string())?;
    let heroic = match diff {
        "normal" | "" => false,
        "heroic" => true,
        _ => return Err("The difficulty is normal or heroic.".into()),
    };
    if heroic && !raid.heroic {
        return Err(format!("{title} has no heroic: play it on normal."));
    }
    if size != 10 && size != 25 {
        return Err("A raid is 10 or 25 players.".into());
    }
    let party = Party::of(party, companions).ok_or_else(|| "Play with companions or with friends.".to_string())?;
    let (_, _, specs) = CLASSES.iter().find(|c| c.0 == class).ok_or_else(|| "Pick a class for your hero.".to_string())?;
    if !specs.iter().any(|s| s.0 == spec) {
        return Err("Pick one of that class's specs.".into());
    }
    Ok(StartPick { start: key.into(), scope, heroic, size, party, class: class.into(), spec: spec.into() })
}

/// `bossfight hero go <ACCOUNT> <start> <difficulty> <mode> <class> <spec>`: the hero waits at that start.
pub fn start_go_line(account: &str, p: &StartPick) -> String {
    format!("bossfight hero go {} {} {} {} {} {}", account.to_ascii_uppercase(), p.start, p.diff_token(), p.party.mode(), p.class, p.spec)
}

/// The role of a spec (the icons): "tank", "heal" or "dps".
pub fn role_of(class: &str, spec: &str) -> &'static str {
    CLASSES.iter().find(|c| c.0 == class).and_then(|c| c.2.iter().find(|s| s.0 == spec)).map(|s| s.2).unwrap_or("dps")
}

/// The companions a raid of `size` brings for these players' roles (BossfightsLogic.h DefaultSeats / DefaultSeats25 and
/// BotSeats: each player takes the first seat of their role, a role with no seat left takes the last damage seat):
/// (tanks, healers, damage). `fill` false: none.
pub fn companions_for(size: u8, roles: &[&str], fill: bool) -> (u8, u8, u8) {
    if !fill {
        return (0, 0, 0);
    }
    let mut seats: Vec<&str> = if size == 25 {
        [vec!["tank"; 3], vec!["heal"; 6], vec!["dps"; 16]].concat()
    } else {
        [vec!["tank"; 2], vec!["heal"; 3], vec!["dps"; 5]].concat()
    };
    for r in roles {
        if seats.is_empty() {
            break;
        }
        match seats.iter().position(|s| s == r) {
            Some(i) => {
                seats.remove(i);
            }
            None => {
                let i = seats.iter().rposition(|s| *s == "dps").unwrap_or(seats.len() - 1);
                seats.remove(i);
            }
        }
    }
    seats.truncate((size as usize).saturating_sub(roles.len()));
    let n = |w: &str| seats.iter().filter(|s| **s == w).count() as u8;
    (n("tank"), n("heal"), n("dps"))
}

/// The hero account of a login (BossfightsHeroLogic.h HeroAccountName): `<LOGIN>BF`, or for a login too long for that
/// `BF` + 14 hex digits of its FNV-1a hash. None for a name that isn't a plain account name.
pub fn hero_account_of(main: &str) -> Option<String> {
    if main.is_empty() || main.len() > 16 || !main.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let u = main.to_ascii_uppercase();
    if u.len() + 2 <= 16 {
        return Some(format!("{u}BF"));
    }
    let mut h: u64 = 1469598103934665603;
    for b in u.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    Some(format!("BF{:014X}", h & 0x00FF_FFFF_FFFF_FFFF))
}

/// The console words Boss Rush's page and its playtest send (`bossfight rush <verb> <ACCOUNT> [arg]`).
pub fn rush_line(verb: &str, account: &str, arg: Option<&str>) -> Result<String, String> {
    if !["status", "retry", "go", "pull", "autopilot", "leave"].contains(&verb) {
        return Err("That isn't a Boss Rush word.".into());
    }
    if account.is_empty() || account.len() > 16 || !account.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("That login doesn't look right.".into());
    }
    let arg = match arg {
        None => String::new(),
        Some(a) if ["on", "off", "nobots"].contains(&a) => format!(" {a}"),
        Some(_) => return Err("That word doesn't go with it.".into()),
    };
    Ok(format!("bossfight rush {verb} {}{arg}", account.to_ascii_uppercase()))
}

/// The module's answer to `bossfight rush ...` (BossfightsStarts.h RushLine and the bfrush ok / no lines).
#[derive(Debug, Clone, PartialEq)]
pub enum Rush {
    Run(RushRun),
    Lobby { start: String, diff: String, waiting: u32, size: u32 },
    Offline,
    Ok { word: String },
    No { why: String },
    Nothing,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RushRun {
    pub start: String,
    pub diff: String,
    pub phase: String,
    pub leader: String,
    pub map: u32,
    pub instance: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// The hero's distance from the start it should stand at (yards).
    pub dist: f32,
    pub members: u32,
    pub bots: u32,
    pub inside: u32,
    /// The start the raid stands at now (a retry's boss).
    pub step: String,
    pub done: u32,
    pub of: u32,
    /// The boss's state in the instance script (0 not started, 1 in progress, 2 failed, 3 done).
    pub boss: u32,
    pub wipes: u32,
    pub clears: u32,
    /// The boss's phase casts seen in this attempt.
    pub marks: Vec<String>,
    pub last: String,
    pub ms: u64,
}

pub fn parse_rush(text: &str) -> Rush {
    let clean = |s: &str, n: usize| s.chars().filter(|c| c.is_ascii_alphanumeric() || " .,:;'()/_-".contains(*c)).take(n).collect::<String>();
    for line in text.replace('\r', "\n").split('\n') {
        let line = line.trim();
        if line == "bfrush offline" {
            return Rush::Offline;
        }
        if let Some(w) = line.strip_prefix("bfrush ok ") {
            return Rush::Ok { word: clean(w, 60) };
        }
        if let Some(w) = line.strip_prefix("bfrush no ") {
            return Rush::No { why: clean(w, 160) };
        }
        let kv = |rest: &str| -> Vec<(String, String)> { rest.split(' ').filter_map(|t| t.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect() };
        if let Some(rest) = line.strip_prefix("bfrush lobby ") {
            let m = kv(rest);
            let g = |k: &str| m.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
            return Rush::Lobby { start: clean(&g("key"), 24), diff: clean(&g("diff"), 12), waiting: g("waiting").parse().unwrap_or(0), size: g("size").parse().unwrap_or(10) };
        }
        if let Some(rest) = line.strip_prefix("bfrush run ") {
            let m = kv(rest);
            let g = |k: &str| m.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
            let num = |k: &str| g(k).parse::<u32>().unwrap_or(0);
            let fl = |k: &str| g(k).parse::<f32>().unwrap_or(0.0);
            let word = |k: &str| {
                let v = clean(&g(k), 40);
                if v == "-" { String::new() } else { v }
            };
            let (done, of) = g("done").split_once('/').map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(0))).unwrap_or((0, 0));
            return Rush::Run(RushRun {
                start: word("key"),
                diff: word("diff"),
                phase: word("phase"),
                leader: word("leader"),
                map: num("map"),
                instance: num("inst"),
                x: fl("x"),
                y: fl("y"),
                z: fl("z"),
                dist: fl("dist"),
                members: num("members"),
                bots: num("bots"),
                inside: num("inside"),
                step: word("step"),
                done,
                of,
                boss: num("boss"),
                wipes: num("wipes"),
                clears: num("clears"),
                marks: word("marks").split(',').filter(|m| !m.is_empty()).map(str::to_string).collect(),
                last: word("last"),
                ms: g("ms").parse().unwrap_or(0),
            });
        }
    }
    Rush::Nothing
}

/// Every raid map the catalog plays on: what the exact engine reads from the game folder (map_extractor `-m`).
pub fn maps() -> Vec<u32> {
    let mut v: Vec<u32> = FIGHTS.iter().map(|f| f.map).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The `-m` argument: "249,615,616,624,631,724".
pub fn maps_arg() -> String {
    maps().iter().map(|m| m.to_string()).collect::<Vec<_>>().join(",")
}

/// The eight start-area tiles the world server checks before it starts (the core's World.cpp; the same list as
/// realm/custom/patches/core.patch): (map, grid x, grid y), the file names `<map:03><gx:02><gy:02>.map`.
pub fn start_tiles() -> Vec<(u32, u32, u32)> {
    const AT: [(u32, f32, f32); 8] = [
        (0, -6240.32, 331.033),
        (0, -8949.95, -132.493),
        (1, -618.518, -4251.67),
        (0, 1676.35, 1677.45),
        (1, 10311.3, 832.463),
        (1, -2917.58, -257.98),
        (530, 10349.6, -6357.29),
        (530, -3961.64, -13931.2),
    ];
    AT.iter().map(|&(m, x, y)| (m, (32.0 - x / 533.33333).max(0.0) as u32, (32.0 - y / 533.33333).max(0.0) as u32)).collect()
}

/// A map file the exact engine keeps (`maps/<map:03><gx:02><gy:02>.map`): one of the catalog's maps, or a start
/// tile. Used to trim a full extraction when the engine's extractor is older than `-m`.
pub fn keeps_map_file(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".map") else { return false };
    if stem.len() != 7 || !stem.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let (m, gx, gy) = (stem[..3].parse::<u32>().unwrap_or(9999), stem[3..5].parse::<u32>().unwrap_or(99), stem[5..].parse::<u32>().unwrap_or(99));
    maps().contains(&m) || start_tiles().contains(&(m, gx, gy))
}

pub fn fight(key: &str) -> Option<&'static Fight> {
    FIGHTS.iter().find(|f| f.key == key)
}

/// The engine a fight is played on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Exact,
    Light,
}

impl Engine {
    pub fn word(self) -> &'static str {
        match self {
            Engine::Exact => "exact",
            Engine::Light => "light",
        }
    }
    pub fn of(w: &str) -> Option<Engine> {
        match w {
            "exact" => Some(Engine::Exact),
            "light" => Some(Engine::Light),
            _ => None,
        }
    }
}

/// Classes and their specs as the module's heroes take them (BossfightsHeroLogic.h Specs(); the class words of
/// BossfightsLogic.h ClassToken): (class word, label, [(spec word, label, role)]).
pub const CLASSES: &[(&str, &str, &[(&str, &str, &str)])] = &[
    ("warrior", "Warrior", &[("arms", "Arms", "dps"), ("fury", "Fury", "dps"), ("prot", "Protection", "tank")]),
    ("paladin", "Paladin", &[("holy", "Holy", "heal"), ("prot", "Protection", "tank"), ("ret", "Retribution", "dps")]),
    ("hunter", "Hunter", &[("bm", "Beast Mastery", "dps"), ("mm", "Marksmanship", "dps"), ("sv", "Survival", "dps")]),
    ("rogue", "Rogue", &[("assa", "Assassination", "dps"), ("combat", "Combat", "dps"), ("sub", "Subtlety", "dps")]),
    ("priest", "Priest", &[("disc", "Discipline", "heal"), ("holy", "Holy", "heal"), ("shadow", "Shadow", "dps")]),
    ("deathknight", "Death Knight", &[("blood", "Blood", "tank"), ("frost", "Frost", "dps"), ("unholy", "Unholy", "dps")]),
    ("shaman", "Shaman", &[("ele", "Elemental", "dps"), ("enh", "Enhancement", "dps"), ("resto", "Restoration", "heal")]),
    ("mage", "Mage", &[("arcane", "Arcane", "dps"), ("fire", "Fire", "dps"), ("frost", "Frost", "dps")]),
    ("warlock", "Warlock", &[("affli", "Affliction", "dps"), ("demo", "Demonology", "dps"), ("destro", "Destruction", "dps")]),
    ("druid", "Druid", &[("balance", "Balance", "dps"), ("bear", "Feral (bear)", "tank"), ("resto", "Restoration", "heal"), ("cat", "Feral (cat)", "dps")]),
];

/// What the player picked on the tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub fight: String,
    pub heroic: bool,
    pub class: String,
    pub spec: String,
    pub engine: Engine,
}

impl Pick {
    pub fn diff(&self) -> &'static str {
        if self.heroic {
            "heroic"
        } else {
            "normal"
        }
    }
}

/// A pick checked against the catalog: the fight, heroic only where the raid has one, a class and one of its specs,
/// the light engine only for a fight that has a light version.
pub fn check(fight_key: &str, diff: &str, class: &str, spec: &str, engine: &str) -> Result<Pick, String> {
    let f = fight(fight_key).ok_or_else(|| "Pick a fight from the list.".to_string())?;
    let heroic = match diff {
        "normal" | "" => false,
        "heroic" => true,
        _ => return Err("The difficulty is normal or heroic.".into()),
    };
    if heroic && !f.heroic {
        return Err(format!("{} has no heroic: play it on normal.", f.title));
    }
    let (_, _, specs) = CLASSES.iter().find(|c| c.0 == class).ok_or_else(|| "Pick a class for your hero.".to_string())?;
    if !specs.iter().any(|s| s.0 == spec) {
        return Err("Pick one of that class's specs.".into());
    }
    let engine = Engine::of(if engine.is_empty() { "exact" } else { engine }).ok_or_else(|| "The engine is exact or light.".to_string())?;
    if engine == Engine::Light && !f.light {
        return Err(format!("{} has no light version yet: play it on the exact engine.", f.title));
    }
    Ok(Pick { fight: f.key.into(), heroic, class: class.into(), spec: spec.into(), engine })
}

// ------------------------------------------------------------------------------------------------ the console lines

/// The player's own account on the Boss Rush install (engine.rs Kind::BossRush) and its hero account
/// (BossfightsHeroLogic.h HeroAccountName: the name + "BF").
pub const ACCOUNT: &str = "RUSH";
pub fn hero_account(main: &str) -> String {
    format!("{}BF", main.to_ascii_uppercase())
}

/// The console lines that make the hero account's login known to the launcher: create it (an error when it is
/// there already is fine), then set the password. Account names and passwords are letters and digits only.
pub fn account_lines(hero: &str, pass: &str) -> Result<[String; 2], String> {
    let ok = |s: &str| !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_alphanumeric());
    if !ok(hero) || !ok(pass) {
        return Err("That login doesn't look right.".into());
    }
    Ok([format!("account create {hero} {pass}"), format!("account set password {hero} {pass} {pass}")])
}

/// `bossfight hero go <ACCOUNT> <boss> <difficulty> bots <class> <spec>`: the module makes (or picks) the hero, keeps
/// the fight as its waiting go, and its next login lands on that fight's platform.
pub fn go_line(account: &str, p: &Pick) -> String {
    format!("bossfight hero go {} {} {} bots {} {}", account.to_ascii_uppercase(), p.fight, p.diff(), p.class, p.spec)
}

/// The module's answer to `hero go`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeroReply {
    Ok { name: String, class: String, spec: String, spec2: String, made: bool, account: String },
    No(String),
    Nothing,
}

/// `bfhero ok <Name> <class> <spec> <spec2> <made|ready> <HERO ACCOUNT>` or `bfhero no <why>` (stream/bossfights.py
/// parse_hero_reply reads the same lines), anywhere in the console's text.
pub fn parse_reply(text: &str) -> HeroReply {
    for line in text.replace('\r', "\n").split('\n') {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("bfhero ok ") {
            let w: Vec<&str> = rest.split(' ').collect();
            let name_ok = |n: &str| (2..=12).contains(&n.len()) && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && n.chars().skip(1).all(|c| c.is_ascii_lowercase());
            let word_ok = |s: &str| (2..=11).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase());
            if w.len() == 6 && name_ok(w[0]) && CLASSES.iter().any(|c| c.0 == w[1]) && word_ok(w[2]) && word_ok(w[3]) && (w[4] == "made" || w[4] == "ready")
                && (1..=16).contains(&w[5].len()) && w[5].bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            {
                return HeroReply::Ok { name: w[0].into(), class: w[1].into(), spec: w[2].into(), spec2: w[3].into(), made: w[4] == "made", account: w[5].into() };
            }
        }
        if let Some(why) = line.strip_prefix("bfhero no ") {
            let why: String = why.chars().filter(|c| c.is_ascii_alphanumeric() || " .,:;'()/-".contains(*c)).take(120).collect();
            return HeroReply::No(if why.trim().is_empty() { "the realm said no".into() } else { why.trim().into() });
        }
    }
    HeroReply::Nothing
}

/// The text of a SOAP answer (`<result>...</result>`, entities undone), or of its fault (`<faultstring>`).
pub fn soap_text(body: &str) -> String {
    let pick = |tag: &str| -> Option<String> {
        let a = body.find(&format!("<{tag}>"))? + tag.len() + 2;
        let b = body[a..].find(&format!("</{tag}>"))? + a;
        Some(body[a..b].to_string())
    };
    let t = pick("result").or_else(|| pick("faultstring")).unwrap_or_default();
    t.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&#xD;", "\r").replace("&#13;", "\r").replace("&amp;", "&")
}

// ------------------------------------------------------------------------------------------------ the board

/// One finished run, kept on this PC (`<app>/bossrush/board.tsv`: one line each, tab separated).
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub fight: String,
    pub heroic: bool,
    pub engine: Engine,
    pub class: String,
    pub spec: String,
    pub ms: u64,
    pub cleared: bool,
    pub at: u64,
}

/// At most this many lines are kept (the oldest wipes go first).
pub const BOARD_MAX: usize = 2000;

pub fn run_line(r: &Run) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", r.fight, if r.heroic { "heroic" } else { "normal" }, r.engine.word(), r.class, r.spec, r.ms, if r.cleared { "clear" } else { "wipe" }, r.at)
}

/// The board's lines read back; anything that doesn't read is skipped (a hand-edited file never stops the page).
pub fn read_board(text: &str) -> Vec<Run> {
    text.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split('\t').collect();
            if w.len() != 8 || fight(w[0]).is_none() {
                return None;
            }
            Some(Run {
                fight: w[0].into(),
                heroic: w[1] == "heroic",
                engine: Engine::of(w[2])?,
                class: w[3].chars().filter(|c| c.is_ascii_lowercase() || *c == '-').take(12).collect(),
                spec: w[4].chars().filter(|c| c.is_ascii_lowercase() || *c == '-').take(12).collect(),
                ms: w[5].parse().ok()?,
                cleared: w[6] == "clear",
                at: w[7].parse().ok()?,
            })
        })
        .collect()
}

/// The board with one more run (trimmed to [`BOARD_MAX`]: the oldest wipes first, then the oldest).
pub fn add_run(text: &str, r: &Run) -> String {
    let mut v = read_board(text);
    v.push(r.clone());
    while v.len() > BOARD_MAX {
        match v.iter().position(|x| !x.cleared) {
            Some(i) => v.remove(i),
            None => v.remove(0),
        };
    }
    let mut out = String::new();
    for r in &v {
        let _ = writeln!(out, "{}", run_line(r));
    }
    out
}

/// The best clear of each fight, difficulty and engine (fastest first within each), and whether a run is a new best.
pub fn bests(runs: &[Run]) -> Vec<Run> {
    let mut best: Vec<Run> = Vec::new();
    for r in runs.iter().filter(|r| r.cleared) {
        match best.iter_mut().find(|b| b.fight == r.fight && b.heroic == r.heroic && b.engine == r.engine) {
            Some(b) if r.ms < b.ms => *b = r.clone(),
            Some(_) => {}
            None => best.push(r.clone()),
        }
    }
    best.sort_by(|a, b| FIGHTS.iter().position(|f| f.key == a.fight).cmp(&FIGHTS.iter().position(|f| f.key == b.fight)).then(a.heroic.cmp(&b.heroic)).then(a.ms.cmp(&b.ms)));
    best
}

pub fn is_new_best(runs: &[Run], r: &Run) -> bool {
    r.cleared && !runs.iter().any(|x| x.cleared && x.fight == r.fight && x.heroic == r.heroic && x.engine == r.engine && x.ms <= r.ms && x != r)
}

/// m:ss.t, as the in-game panel shows it.
pub fn clock(ms: u64) -> String {
    let t = ms / 100;
    format!("{}:{:02}.{}", t / 600, (t / 10) % 60, t % 10)
}

/// The module's result line on the in-game panel (`R clear|wipe <ms> <rank|->`, docs/BOSS-FIGHTS.md), as the
/// launcher reads it from the game's chat log: (cleared, ms).
pub fn parse_result(line: &str) -> Option<(bool, u64)> {
    let w: Vec<&str> = line.trim().split('\t').collect();
    if w.len() >= 3 && w[0] == "R" && (w[1] == "clear" || w[1] == "wipe") {
        return Some((w[1] == "clear", w[2].parse().ok()?));
    }
    None
}

/// The exact engine's own record of every attempt (the module's `characters.leyline_bossfight_runs`), read after the
/// last one the board has: id, fight, difficulty, ms, result, ended, roster.
pub fn runs_sql(after_id: u64) -> String {
    format!("SELECT id, boss, difficulty, ms, result, ended_at, roster FROM leyline_bossfight_runs WHERE id > {after_id} AND mode = 'bots' ORDER BY id LIMIT 200;")
}

/// Those rows (the mysql client's `-N -B` output: tab separated) as board runs, with their ids. The hero's class is
/// the roster's first player (`Name:class:player`); the spec isn't recorded there ("-").
pub fn parse_db_runs(text: &str) -> Vec<(u64, Run)> {
    text.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split('\t').collect();
            if w.len() < 7 || fight(w[1]).is_none() || (w[4] != "clear" && w[4] != "wipe") {
                return None;
            }
            let class = w[6].split(',').find_map(|e| {
                let p: Vec<&str> = e.split(':').collect();
                (p.len() == 3 && p[2] == "player" && CLASSES.iter().any(|c| c.0 == p[1])).then(|| p[1].to_string())
            });
            Some((w[0].parse().ok()?, Run {
                fight: w[1].into(),
                heroic: w[2] == "heroic",
                engine: Engine::Exact,
                class: class.unwrap_or_else(|| "-".into()),
                spec: "-".into(),
                ms: w[3].parse().ok()?,
                cleared: w[4] == "clear",
                at: w[5].parse().ok()?,
            }))
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------ the share code

/// A fight setup as a code a friend pastes (the Den codes' envelope, labs/den-lite denpack.rs, with kind `fight`):
/// `ley1.fight.r.<base64url of the text>.<crc32>`, the text `boss=<key>;diff=<normal|heroic>;class=<w>;spec=<w>;engine=<w>`.
pub const CODE_PREFIX: &str = "ley1.fight.r.";

pub fn share_code(p: &Pick) -> String {
    let text = format!("boss={};diff={};class={};spec={};engine={}", p.fight, p.diff(), p.class, p.spec, p.engine.word());
    format!("{CODE_PREFIX}{}.{:08x}", b64url(text.as_bytes()), crc32(text.as_bytes()))
}

/// A code read back and checked like a pick (nothing in a code runs; it only names a catalog fight and a hero).
pub fn read_code(code: &str) -> Result<Pick, String> {
    let code = code.trim();
    if code.len() > 400 {
        return Err("That code is too long to be a fight code.".into());
    }
    let rest = code.strip_prefix(CODE_PREFIX).ok_or_else(|| "That isn't a Boss Rush code.".to_string())?;
    let (body, check_hex) = rest.rsplit_once('.').ok_or_else(|| "That code is cut short.".to_string())?;
    let bytes = un_b64url(body).ok_or_else(|| "That code is damaged.".to_string())?;
    if format!("{:08x}", crc32(&bytes)) != check_hex {
        return Err("That code is damaged (its check doesn't match).".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "That code is damaged.".to_string())?;
    let get = |k: &str| text.split(';').find_map(|kv| kv.strip_prefix(&format!("{k}=")).map(str::to_string)).unwrap_or_default();
    let (b, d, c, s, e) = (get("boss"), get("diff"), get("class"), get("spec"), get("engine"));
    check(&b, &d, &c, &s, &e)
}

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url(b: &[u8]) -> String {
    let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            o.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    o
}

fn un_b64url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let vals: Vec<u32> = s.bytes().map(|c| B64.iter().position(|x| *x == c).map(|p| p as u32)).collect::<Option<_>>()?;
    for ch in vals.chunks(4) {
        if ch.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (i, v) in ch.iter().enumerate() {
            n |= v << (18 - 6 * i);
        }
        for i in 0..ch.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// No game brand word in anything the tile shows (the rules: the legal line is the only exception, and it isn't here).
/// Monday 5 Jan 1970 00:00 UTC: weeks count from here (the boards' week, realm/custom/boards-export.py WEEK0).
pub const WEEK0: u64 = 4 * 86_400;
pub const WEEK: u64 = 7 * 86_400;

/// This week's featured fight: the catalog in turn, one a week from Monday 00:00 UTC (the same rule as
/// boards-export.py `featured` and ov_boards.lua `B.featured`; docs/LEADERBOARDS.md).
/// The fights the boards and the weekly featured fight turn through: the first ones in [`FIGHTS`], the same list as
/// boards-export.py CATALOG and ov_boards.lua. The single-boss starts added after them (start anywhere: `icc-*`) are
/// places to begin a run, not boards of their own, so the featured week stays the same when one is added.
pub const BOARD_FIGHTS: usize = 12;

pub fn featured_fight(now: u64) -> &'static Fight {
    let n = now.saturating_sub(WEEK0) / WEEK;
    &FIGHTS[(n % BOARD_FIGHTS as u64) as usize]
}

/// Monday 00:00 UTC of `now`'s week.
pub fn week_start(now: u64) -> u64 {
    WEEK0 + now.saturating_sub(WEEK0) / WEEK * WEEK
}

pub fn brand_free(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    !["warcraft", "lich king", "blizzard"].iter().any(|w| l.contains(w))
        && !l.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "wow")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module's table, read at test time: the realm folder is in the workspace, not in the public Leyline repo
    /// (its export leaves realm/ out, and refuses a launcher that builds a file from there in). None there: skipped.
    fn logic() -> Option<String> {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/custom/modules/mod-leyline-bossfights/src/BossfightsLogic.h")).ok()
    }

    /// The module's catalog rows: (key, title, raid, map, heroic).
    fn module_rows(logic: &str) -> Vec<(String, String, String, u32, bool)> {
        let mut v = Vec::new();
        for line in logic.lines() {
            let t = line.trim();
            if !t.starts_with("{ \"") {
                continue;
            }
            // three quoted strings (a title may hold a comma), then the numbers
            let q: Vec<&str> = t.split('"').collect();
            if q.len() < 7 {
                continue;
            }
            let parts: Vec<&str> = q[6].split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
            if parts.len() < 7 {
                continue;
            }
            let Ok(map) = parts[0].parse::<u32>() else { continue };
            v.push((q[1].to_string(), q[3].to_string(), q[5].to_string(), map, parts[6] == "true"));
        }
        v
    }

    #[test]
    fn the_catalog_is_the_modules_own() {
        let Some(logic) = logic() else { return };
        let rows = module_rows(&logic);
        assert_eq!(rows.len(), FIGHTS.len(), "{rows:?}");
        for (f, r) in FIGHTS.iter().zip(&rows) {
            assert_eq!((f.key, f.title, f.raid, f.map, f.heroic), (r.0.as_str(), r.1.as_str(), r.2.as_str(), r.3, r.4));
        }
        assert_eq!(FIGHTS[0].key, "frozen-throne", "the featured fight first");
    }

    fn starts_h() -> Option<String> {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/custom/modules/mod-leyline-bossfights/src/BossfightsStarts.h")).ok()
    }

    #[test]
    fn the_raid_tree_is_the_modules_own() {
        let Some(h) = starts_h() else { return };
        let mut in_raids = 0;
        for r in RAIDS {
            assert!(h.contains(&format!("{{ \"{}\", \"{}\", {}, ", r.key, r.title, r.map)), "{r:?}");
            // the raid's block: its fights, in order
            let at = h.find(&format!("{{ \"{}\", \"{}\"", r.key, r.title)).unwrap();
            let block = &h[at..];
            let list = block.split('{').nth(2).unwrap_or("");
            let keys: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
            assert_eq!(keys, r.fights.to_vec(), "{}", r.key);
            for f in r.fights {
                assert_eq!(fight(f).map(|x| x.map), Some(r.map), "{f}");
            }
            in_raids += r.fights.len();
            for w in r.wings.iter().filter(|w| !w.key.is_empty()) {
                assert!(h.contains(&format!("{{ \"{}\", \"{}\", \"{}\"", w.key, w.title, w.fights[0])), "{w:?}");
                assert!(w.key.len() <= 17);
            }
            if !r.whole.is_empty() {
                assert!(h.contains(&format!("\"{}\", {{", r.whole)), "{}", r.whole);
            }
            assert!(brand_free(r.title) && r.wings.iter().all(|w| brand_free(w.title)));
        }
        assert_eq!(in_raids, FIGHTS.len(), "every fight is in one raid");
    }

    #[test]
    fn starts_are_checked() {
        assert_eq!(start("icc-plague").map(|s| (s.0, s.1, s.2.key)), Some((Scope::Wing, "The Plagueworks", "icc")));
        assert_eq!(start("icc-raid").map(|s| s.0), Some(Scope::Raid));
        assert_eq!(start("icc-council").map(|s| (s.0, s.1)), Some((Scope::Boss, "The Blood Council")));
        assert_eq!(start("frozen-throne").map(|s| s.0), Some(Scope::Boss), "a wing of one boss plays the boss");
        assert!(start("").is_none() && start("os-raid").is_none() && start("nope").is_none());
        let p = check_start("icc-frostwing", "heroic", 25, "friends", true, "priest", "holy").unwrap();
        assert_eq!((p.diff_token(), p.party.mode(), p.scope), ("heroic25".to_string(), "group", Scope::Wing));
        assert_eq!(start_go_line("rush", &p), "bossfight hero go RUSH icc-frostwing heroic25 group priest holy");
        let solo = check_start("icc-council", "normal", 10, "companions", true, "warrior", "prot").unwrap();
        assert_eq!(start_go_line("RUSH", &solo), "bossfight hero go RUSH icc-council normal bots warrior prot");
        assert_eq!(check_start("icc-raid", "normal", 10, "friends", false, "mage", "fire").unwrap().party.mode(), "group-only");
        assert!(check_start("voa-raid", "heroic", 10, "companions", true, "mage", "fire").unwrap_err().contains("no heroic"));
        assert!(check_start("icc-raid", "normal", 20, "companions", true, "mage", "fire").is_err());
        assert!(check_start("icc-raid", "normal", 10, "strangers", true, "mage", "fire").is_err());
        assert!(check_start("icc-raid", "normal", 10, "companions", true, "mage", "bear").is_err());
        assert!(check_start("icc-raid; .server shutdown", "normal", 10, "companions", true, "mage", "fire").is_err());
    }

    #[test]
    fn companions_fill_the_roles() {
        assert_eq!(companions_for(10, &["dps"], true), (2, 3, 4));
        assert_eq!(companions_for(10, &["tank"], true), (1, 3, 5));
        assert_eq!(companions_for(25, &["heal"], true), (3, 5, 16));
        assert_eq!(companions_for(10, &["tank", "dps", "heal"], true), (1, 2, 4));
        assert_eq!(companions_for(10, &["tank", "tank", "tank"], true), (0, 3, 4), "a third tank takes a damage seat");
        assert_eq!(companions_for(10, &["dps"], false), (0, 0, 0));
        assert_eq!(role_of("paladin", "prot"), "tank");
        assert_eq!(role_of("druid", "resto"), "heal");
        // the module's own seats: the same counts (BossfightsLogic.h)
        if let Some(l) = logic() {
            let seats = |name: &str| -> (usize, usize, usize) {
                let body = l.split(&format!("inline std::vector<Seat> {name}()")).nth(1).unwrap().split("};").next().unwrap().to_string();
                (body.matches("ROLE_TANK }").count(), body.matches("ROLE_HEAL }").count(), body.matches("ROLE_DPS }").count())
            };
            assert_eq!(seats("DefaultSeats"), (2, 3, 5));
            assert_eq!(seats("DefaultSeats25"), (3, 6, 16));
        }
    }

    #[test]
    fn hero_accounts_as_the_module_names_them() {
        // the module's own test values (tests/test_bossfights_heroes.cpp)
        assert_eq!(hero_account_of("wr000123").as_deref(), Some("WR000123BF"));
        assert_eq!(hero_account_of("ABCDEFGHIJKLMN").as_deref(), Some("ABCDEFGHIJKLMNBF"));
        assert_eq!(hero_account_of("ABCDEFGHIJKLMNO").as_deref(), Some("BF0263AAD3AE34E1"));
        assert_eq!(hero_account_of("wr0001234567890x").as_deref(), Some("BF96F76C9CEAE255"));
        assert_eq!(hero_account_of("a b"), None);
        assert_eq!(hero_account_of(""), None);
        assert_eq!(hero_account(ACCOUNT), hero_account_of(ACCOUNT).unwrap());
    }

    #[test]
    fn the_rush_words_and_answers() {
        assert_eq!(rush_line("status", "rush", None).unwrap(), "bossfight rush status RUSH");
        assert_eq!(rush_line("autopilot", "RUSH", Some("on")).unwrap(), "bossfight rush autopilot RUSH on");
        assert_eq!(rush_line("go", "RUSH", Some("nobots")).unwrap(), "bossfight rush go RUSH nobots");
        assert!(rush_line("shutdown", "RUSH", None).is_err());
        assert!(rush_line("status", "RUSH; x", None).is_err());
        assert!(rush_line("status", "RUSH", Some("now; .server")).is_err());
        let line = "bfrush run key=frozen-throne diff=normal phase=ready leader=Kalinra map=631 inst=7 x=517.0 y=-2124.6 z=840.9 dist=1.2 \
                    members=10 bots=9 inside=10 step=frozen-throne done=0/1 boss=0 wipes=0 clears=0 marks=transition1,quake last=- ms=1234";
        let Rush::Run(r) = parse_rush(&format!("{line}\r\n")) else { panic!("{:?}", parse_rush(line)) };
        assert_eq!((r.start.as_str(), r.phase.as_str(), r.map, r.instance, r.members, r.bots, r.inside, r.done, r.of), ("frozen-throne", "ready", 631, 7, 10, 9, 10, 0, 1));
        assert_eq!((r.dist, r.marks.clone(), r.last.as_str(), r.ms, r.step.as_str()), (1.2, vec!["transition1".to_string(), "quake".to_string()], "", 1234, "frozen-throne"));
        assert_eq!(parse_rush("bfrush lobby key=icc-raid diff=normal25 waiting=2 size=25"), Rush::Lobby { start: "icc-raid".into(), diff: "normal25".into(), waiting: 2, size: 25 });
        assert_eq!(parse_rush("bfrush offline"), Rush::Offline);
        assert_eq!(parse_rush("bfrush ok retry icc-bloat"), Rush::Ok { word: "retry icc-bloat".into() });
        assert_eq!(parse_rush("bfrush no the fight is on <b>"), Rush::No { why: "the fight is on b".into() });
        assert_eq!(parse_rush("There is no such command."), Rush::Nothing);
    }

    #[test]
    fn words_are_clean() {
        for f in FIGHTS {
            assert!(brand_free(f.title) && brand_free(f.raid) && brand_free(f.blurb), "{f:?}");
        }
        for c in CLASSES {
            assert!(brand_free(c.1));
        }
        assert!(!brand_free("the Lich King") && !brand_free("WoW") && brand_free("wowzers") && !brand_free("a wow b"));
    }

    #[test]
    fn maps_and_the_lighter_read() {
        assert_eq!(maps(), vec![249, 615, 616, 624, 631, 724]);
        assert_eq!(maps_arg(), "249,615,616,624,631,724");
        let t = start_tiles();
        assert_eq!(t.len(), 8);
        assert!(t.contains(&(0, 43, 31)), "the first human start: {t:?}");
        assert!(keeps_map_file("6313531.map") && keeps_map_file("0004331.map"));
        assert!(!keeps_map_file("0003232.map") && !keeps_map_file("5710000.map") && !keeps_map_file("631.map") && !keeps_map_file("x.dbc"));
    }

    #[test]
    fn picks_are_checked() {
        let p = check("frozen-throne", "heroic", "mage", "fire", "").unwrap();
        assert_eq!((p.diff(), p.engine), ("heroic", Engine::Exact));
        assert!(check("brood-mother", "heroic", "mage", "fire", "exact").unwrap_err().contains("no heroic"));
        assert!(check("nope", "normal", "mage", "fire", "exact").is_err());
        assert!(check("frozen-throne", "mythic", "mage", "fire", "exact").is_err());
        assert!(check("frozen-throne", "normal", "mage", "bear", "exact").is_err());
        assert!(check("frozen-throne", "normal", "bard", "fire", "exact").is_err());
        assert!(check("spell-weaver", "normal", "druid", "bear", "light").unwrap_err().contains("light version"));
        assert_eq!(check("bone-lord", "normal", "druid", "bear", "light").unwrap().engine, Engine::Light);
    }

    #[test]
    fn the_console_lines_and_answers() {
        let p = check("twilight-destroyer", "heroic", "priest", "disc", "exact").unwrap();
        assert_eq!(go_line("rush", &p), "bossfight hero go RUSH twilight-destroyer heroic bots priest disc");
        assert_eq!(hero_account(ACCOUNT), "RUSHBF");
        let [a, b] = account_lines("RUSHBF", "ABCD2345").unwrap();
        assert_eq!(a, "account create RUSHBF ABCD2345");
        assert_eq!(b, "account set password RUSHBF ABCD2345 ABCD2345");
        assert!(account_lines("RUSH BF", "x").is_err() && account_lines("RUSHBF", "a;b").is_err());
        let xml = "<?xml?><SOAP-ENV:Body><ns1:executeCommandResponse><result>bfhero ok Kalinra priest disc holy made RUSHBF&#xD;\n</result></ns1:executeCommandResponse>";
        assert_eq!(parse_reply(&soap_text(xml)), HeroReply::Ok { name: "Kalinra".into(), class: "priest".into(), spec: "disc".into(), spec2: "holy".into(), made: true, account: "RUSHBF".into() });
        assert_eq!(parse_reply("bfhero no the Ruby Sanctum <x> has no heroic"), HeroReply::No("the Ruby Sanctum x has no heroic".into()));
        assert_eq!(parse_reply("bfhero ok kalinra priest disc holy made RUSHBF"), HeroReply::Nothing, "a name starts upper case");
        assert_eq!(parse_reply(""), HeroReply::Nothing);
        assert_eq!(soap_text("<faultstring>There is no such command.</faultstring>"), "There is no such command.");
    }

    #[test]
    fn the_board_keeps_bests() {
        let r = |f: &str, ms: u64, clear: bool, at: u64| Run { fight: f.into(), heroic: false, engine: Engine::Exact, class: "mage".into(), spec: "fire".into(), ms, cleared: clear, at };
        let mut t = String::new();
        for (i, x) in [r("brood-mother", 300_000, true, 1), r("brood-mother", 250_000, true, 2), r("brood-mother", 90_000, false, 3), r("frozen-throne", 600_000, true, 4)].iter().enumerate() {
            t = add_run(&t, x);
            assert_eq!(read_board(&t).len(), i + 1);
        }
        let b = bests(&read_board(&t));
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].fight.as_str(), b[0].ms), ("frozen-throne", 600_000), "catalog order");
        assert_eq!(b[1].ms, 250_000);
        let runs = read_board(&t);
        assert!(is_new_best(&runs, &r("brood-mother", 200_000, true, 5)));
        assert!(!is_new_best(&runs, &r("brood-mother", 260_000, true, 5)));
        assert!(!is_new_best(&runs, &r("brood-mother", 1, false, 5)), "a wipe is never a best");
        assert_eq!(read_board("junk\nnope\tx\n"), vec![]);
        assert_eq!(clock(312_345), "5:12.3");
        assert_eq!(parse_result("R\tclear\t312345\t-"), Some((true, 312_345)));
        assert_eq!(parse_result("R\twipe\t1000\t-"), Some((false, 1000)));
        assert_eq!(parse_result("T\t5"), None);
        // trimmed: the oldest wipe goes first
        let mut big = String::new();
        for i in 0..BOARD_MAX {
            let _ = writeln!(big, "{}", run_line(&r("stone-watcher", 1000 + i as u64, i != 0, i as u64)));
        }
        let after = read_board(&add_run(&big, &r("stone-watcher", 5, true, 99_999)));
        assert_eq!(after.len(), BOARD_MAX);
        assert!(after.iter().all(|x| x.cleared), "the one wipe went");
    }

    #[test]
    fn the_engines_own_record_is_read() {
        assert!(runs_sql(41).contains("id > 41") && runs_sql(0).contains("mode = 'bots'"));
        let rows = "42\tbrood-mother\tnormal\t251234\tclear\t1762400000\tKalinra:mage:player,Bot:paladin:bot\n43\tnope\tnormal\t1\tclear\t1\tx\n44\tfrozen-throne\theroic\t90000\twipe\t1762400999\tBot:paladin:bot\n";
        let v = parse_db_runs(rows);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].0, v[0].1.class.as_str(), v[0].1.ms, v[0].1.cleared), (42, "mage", 251_234, true));
        assert_eq!((v[1].0, v[1].1.heroic, v[1].1.cleared, v[1].1.class.as_str()), (44, true, false, "-"));
        let t = add_run("", &v[1].1);
        assert_eq!(read_board(&t)[0].class, "-", "the board keeps an unknown class");
    }

    #[test]
    fn the_featured_fight_turns_once_a_week() {
        // Thursday 12 Nov 2026 19:00 UTC; the week began Monday 9 Nov (1794182400)
        let now = 1_794_510_000;
        assert_eq!(week_start(now), 1_794_182_400);
        assert_eq!(week_start(1_794_182_400), 1_794_182_400);
        assert_eq!(week_start(1_794_182_399), 1_794_182_400 - WEEK);
        // the same as boards-export.py featured(1794510000) over the whole catalog (boards-vector.json)
        assert_eq!(featured_fight(now).key, "cult-mistress");
        assert_eq!(featured_fight(1_794_182_400).key, featured_fight(1_794_182_400 + WEEK - 1).key);
        assert_ne!(featured_fight(now).key, featured_fight(now + WEEK).key);
        assert_eq!(featured_fight(now).key, featured_fight(now + WEEK * BOARD_FIGHTS as u64).key);
        // the boards' fights come first; everything after them is a start-anywhere boss
        assert!(FIGHTS[..BOARD_FIGHTS].iter().all(|f| !f.key.starts_with("icc-")));
        assert!(FIGHTS[BOARD_FIGHTS..].iter().all(|f| f.key.starts_with("icc-")));
    }

    #[test]
    fn share_codes_round_trip_and_are_checked() {
        let p = check("cult-mistress", "heroic", "deathknight", "blood", "light").unwrap();
        let c = share_code(&p);
        assert!(c.starts_with("ley1.fight.r.") && c.len() < 200, "{c}");
        assert_eq!(read_code(&c).unwrap(), p);
        assert_eq!(read_code(&format!("  {c}\n")).unwrap(), p);
        let mut bad = c.clone();
        bad.replace_range(14..15, if &c[14..15] == "A" { "B" } else { "A" });
        assert!(read_code(&bad).is_err());
        assert!(read_code("ley1.den.r.abc.00000000").is_err());
        // a code naming a fight or hero that isn't in the catalog is refused like a bad pick
        let text = "boss=../../x;diff=normal;class=mage;spec=fire;engine=exact";
        let forged = format!("{CODE_PREFIX}{}.{:08x}", b64url(text.as_bytes()), crc32(text.as_bytes()));
        assert!(read_code(&forged).is_err());
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(un_b64url(&b64url(b"any bytes \x00\xff")).unwrap(), b"any bytes \x00\xff");
    }
}
