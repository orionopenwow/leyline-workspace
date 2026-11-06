//! Single player's worlds on this PC and their characters (docs/SINGLE-PLAYER-ONE-CLICK.md): the pure parts behind
//! the launcher's Characters page (ui/characters.js, "On this PC" and "Online") and Quick start.
//!
//! * **Characters on this PC:** each local world (host/engine.rs `KINDS`: Single player, the original world, Boss
//!   Rush) keeps its characters in its own database. While a world runs, the launcher reads them ([`chars_sql`],
//!   [`parse_chars`]: the player's own account only, never the bots' or a friend's) and keeps the list beside its
//!   saves (`<world>/state/characters.json`), so the page shows them when the world is stopped too (only one world
//!   runs at a time: they share this PC's ports).
//! * **Online:** the official realm keeps its characters on the realm; the launcher has no account API for them.
//!   The player's own game folder remembers each character it played, per realm (the game's own settings folders,
//!   `WTF/Account/<ACCOUNT>/<Realm>/<Character>/`): [`online_chars`] reads those names, nothing else.
//! * **Quick start:** a level-1 character of a picked race and class made by the game's own create screen (the
//!   hybrid login's `make`, glue/LeylineHybrid.lua), then entered. [`quick_ok`] checks the pick against the world's
//!   era: the original world offers the first era's races and classes only, as its realm does.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Races: (id, the create screen's file name, what the page says, the first era's).
pub const RACES: &[(u8, &str, &str, bool)] = &[
    (1, "Human", "Human", true),
    (2, "Orc", "Orc", true),
    (3, "Dwarf", "Dwarf", true),
    (4, "NightElf", "Night Elf", true),
    (5, "Scourge", "Undead", true),
    (6, "Tauren", "Tauren", true),
    (7, "Gnome", "Gnome", true),
    (8, "Troll", "Troll", true),
    (10, "BloodElf", "Blood Elf", false),
    (11, "Draenei", "Draenei", false),
];

/// Classes: (id, file name, what the page says, the first era's). Death Knights start at 55 elsewhere: not in Quick start.
pub const CLASSES: &[(u8, &str, &str, bool)] = &[
    (1, "WARRIOR", "Warrior", true),
    (2, "PALADIN", "Paladin", true),
    (3, "HUNTER", "Hunter", true),
    (4, "ROGUE", "Rogue", true),
    (5, "PRIEST", "Priest", true),
    (6, "DEATHKNIGHT", "Death Knight", false),
    (7, "SHAMAN", "Shaman", true),
    (8, "MAGE", "Mage", true),
    (9, "WARLOCK", "Warlock", true),
    (11, "DRUID", "Druid", true),
];

/// Which classes each race can be on the create screen (race id: class ids), without Death Knights. The first era's
/// table for its races; the later races as the game's own client offers them.
pub const COMBOS: &[(u8, &[u8])] = &[
    (1, &[1, 2, 4, 5, 8, 9]),
    (2, &[1, 3, 4, 7, 9]),
    (3, &[1, 2, 3, 4, 5]),
    (4, &[1, 3, 4, 5, 11]),
    (5, &[1, 4, 5, 8, 9]),
    (6, &[1, 3, 7, 11]),
    (7, &[1, 4, 8, 9]),
    (8, &[1, 3, 4, 5, 7, 8]),
    (10, &[2, 3, 4, 5, 8, 9]),
    (11, &[1, 2, 3, 5, 7, 8]),
];

pub fn race_label(id: u8) -> &'static str {
    RACES.iter().find(|r| r.0 == id).map(|r| r.2).unwrap_or("")
}

pub fn class_label(id: u8) -> &'static str {
    CLASSES.iter().find(|c| c.0 == id).map(|c| c.2).unwrap_or("")
}

/// Alliance or Horde, for the page's mark.
pub fn side(race: u8) -> &'static str {
    match race {
        1 | 3 | 4 | 7 | 11 => "alliance",
        2 | 5 | 6 | 8 | 10 => "horde",
        _ => "",
    }
}

/// A Quick start pick for a world: `(race file, class file)` when the world offers it. `first_era`: the original
/// world (its realm holds every character in the first era, so the later races aren't there).
pub fn quick_ok(race: u8, class: u8, first_era: bool) -> Result<(&'static str, &'static str), String> {
    let r = RACES.iter().find(|r| r.0 == race).ok_or("Pick a race first.")?;
    let c = CLASSES.iter().find(|c| c.0 == class && c.0 != 6).ok_or("Pick a class first.")?;
    if first_era && !(r.3 && c.3) {
        return Err(format!("The original world has no {} {}s: they came later. Pick another.", r.2, c.2));
    }
    let ok = COMBOS.iter().find(|x| x.0 == race).is_some_and(|x| x.1.contains(&class));
    if !ok {
        return Err(format!("A {} can't be a {}. Pick another class.", r.2, c.2));
    }
    Ok((r.1, c.1))
}

/// The page's choices for one world: every race it offers, each with its classes.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RaceChoice {
    pub id: u8,
    pub label: &'static str,
    pub side: &'static str,
    pub classes: Vec<(u8, &'static str)>,
}

pub fn choices(first_era: bool) -> Vec<RaceChoice> {
    RACES
        .iter()
        .filter(|r| !first_era || r.3)
        .map(|r| RaceChoice {
            id: r.0,
            label: r.2,
            side: side(r.0),
            classes: COMBOS.iter().find(|x| x.0 == r.0).map(|x| x.1.iter().map(|c| (*c, class_label(*c))).collect()).unwrap_or_default(),
        })
        .collect()
}

/// A name for a Quick start character when the player leaves it empty: one of these, then a letter more if it is
/// taken (the game says so on its create screen, and the player picks another there).
pub const NAMES: &[&str] = &["Wren", "Tamsin", "Odo", "Brakka", "Ysolde", "Corwen", "Mirel", "Thane", "Isra", "Kestrel", "Bram", "Lio"];

/// The name to make: the player's own (cleaned to the game's shape), or one of [`NAMES`] not in `taken`.
pub fn name_for(want: &str, taken: &[String]) -> Result<String, String> {
    let w = want.trim();
    if !w.is_empty() {
        let mut n: String = w.chars().filter(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_lowercase();
        if let Some(f) = n.get(0..1).map(|f| f.to_ascii_uppercase()) {
            n.replace_range(0..1, &f);
        }
        return if crate::hybrid::char_name_ok(&n) { Ok(n) } else { Err("A name is 2 to 12 letters (A to Z), nothing else.".into()) };
    }
    let has = |n: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    NAMES.iter().find(|n| !has(n)).map(|n| n.to_string()).ok_or_else(|| "Type a name for your new character.".to_string())
}

// ------------------------------------------------------------------------------------------------ characters

/// One character of a local world, as the page shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalChar {
    pub name: String,
    pub race: u8,
    pub class: u8,
    pub female: bool,
    pub level: u8,
    /// Seconds played.
    pub played: u64,
    /// When it last logged out (unix seconds; 0: never).
    pub last: u64,
    pub race_label: String,
    pub class_label: String,
    pub side: String,
}

/// The query for the characters of these accounts (the player's own: never the bots' accounts, a friend's or a
/// deleted character), oldest first (the game's own character select order).
pub fn chars_sql(accounts: &[&str]) -> Result<String, String> {
    if accounts.is_empty() || !accounts.iter().all(|a| (3..=16).contains(&a.len()) && a.bytes().all(|b| b.is_ascii_alphanumeric())) {
        return Err("That login doesn't look right.".into());
    }
    let list: Vec<String> = accounts.iter().map(|a| format!("'{}'", a.to_ascii_uppercase())).collect();
    Ok(format!(
        "SELECT c.name, c.race, c.class, c.gender, c.level, c.totaltime, c.logout_time FROM characters c \
         JOIN acore_auth.account a ON a.id = c.account WHERE a.username IN ({}) AND c.deleteInfos_Name IS NULL ORDER BY c.guid",
        list.join(", ")
    ))
}

/// The mysql client's `-N -B` rows of [`chars_sql`]; a row that doesn't look right is left out.
pub fn parse_chars(rows: &str) -> Vec<LocalChar> {
    let mut v = Vec::new();
    for line in rows.lines() {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        if f.len() < 7 || !crate::hybrid::char_name_ok(f[0]) {
            continue;
        }
        let n = |i: usize| f[i].trim().parse::<u64>().ok();
        let (Some(race), Some(class), Some(gender), Some(level)) = (n(1), n(2), n(3), n(4)) else { continue };
        let (race, class) = (race as u8, class as u8);
        v.push(LocalChar {
            name: f[0].to_string(),
            race,
            class,
            female: gender == 1,
            level: level.min(255) as u8,
            played: n(5).unwrap_or(0),
            last: n(6).unwrap_or(0),
            race_label: race_label(race).into(),
            class_label: class_label(class).into(),
            side: side(race).into(),
        });
    }
    v
}

/// The list kept beside a world's saves.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kept {
    /// When it was read from the world (unix seconds).
    pub at: u64,
    pub chars: Vec<LocalChar>,
}

pub fn read_kept(file: &Path) -> Kept {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn write_kept(file: &Path, k: &Kept) -> Result<(), String> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(file, serde_json::to_string_pretty(k).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------------ online

/// A character the player's game folder remembers playing on a realm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnlineChar {
    pub name: String,
    pub realm: String,
}

/// The characters this game folder played on these realms (its own settings folders: `WTF/Account/<ACCOUNT>/
/// <Realm>/<Character>/`, any case). Only folder names are read; each name must be a character's name's shape.
pub fn online_chars(client: &Path, realms: &[&str]) -> Vec<OnlineChar> {
    let mut out: Vec<OnlineChar> = Vec::new();
    let Some(acc) = crate::clients::find_rel(client, "WTF/Account") else { return out };
    let Ok(accounts) = std::fs::read_dir(&acc) else { return out };
    for a in accounts.flatten().filter(|e| e.path().is_dir()) {
        let Ok(rs) = std::fs::read_dir(a.path()) else { continue };
        for r in rs.flatten().filter(|e| e.path().is_dir()) {
            let rname = r.file_name().to_string_lossy().to_string();
            let Some(realm) = realms.iter().find(|x| x.eq_ignore_ascii_case(&rname)) else { continue };
            let Ok(cs) = std::fs::read_dir(r.path()) else { continue };
            for c in cs.flatten().filter(|e| e.path().is_dir()) {
                let name = c.file_name().to_string_lossy().to_string();
                if crate::hybrid::char_name_ok(&name) && !out.iter().any(|o| o.name == name && o.realm == *realm) {
                    out.push(OnlineChar { name, realm: realm.to_string() });
                }
            }
        }
    }
    out.sort_by(|a, b| a.realm.cmp(&b.realm).then(a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_original_world_offers_only_the_first_eras_races_and_classes() {
        assert_eq!(quick_ok(4, 11, true).unwrap(), ("NightElf", "DRUID"));
        assert_eq!(quick_ok(5, 8, true).unwrap(), ("Scourge", "MAGE"));
        assert!(quick_ok(10, 2, true).unwrap_err().contains("came later"), "no later races");
        assert!(quick_ok(10, 2, false).is_ok(), "Single player has them");
        assert!(quick_ok(1, 6, false).is_err(), "no Death Knights in Quick start");
        assert!(quick_ok(6, 8, true).unwrap_err().contains("can't be"), "no Tauren mages");
        assert!(quick_ok(99, 1, true).is_err() && quick_ok(1, 99, true).is_err());
        let c = choices(true);
        assert_eq!(c.len(), 8);
        assert!(c.iter().all(|r| !r.classes.iter().any(|x| x.0 == 6 || x.0 == 10)));
        assert!(c.iter().all(|r| r.classes.iter().all(|x| quick_ok(r.id, x.0, true).is_ok())), "every offered pick is allowed");
        assert_eq!(choices(false).len(), 10);
        // every combination in the table is one the create screen offers its race
        for (race, classes) in COMBOS {
            for cl in classes.iter() {
                assert!(quick_ok(*race, *cl, false).is_ok(), "{race} {cl}");
            }
        }
    }

    #[test]
    fn names_are_the_games_shape() {
        assert_eq!(name_for("  tamsin ", &[]).unwrap(), "Tamsin");
        assert_eq!(name_for("O'do", &[]).unwrap(), "Odo");
        assert!(name_for("a", &[]).is_err());
        assert!(name_for("thirteenletters", &[]).is_err());
        assert_eq!(name_for("", &[]).unwrap(), "Wren");
        assert_eq!(name_for("", &["wren".into(), "Tamsin".into()]).unwrap(), "Odo");
    }

    #[test]
    fn the_players_own_characters_only() {
        let q = chars_sql(&["solo"]).unwrap();
        assert!(q.contains("a.username IN ('SOLO')") && q.contains("deleteInfos_Name IS NULL") && q.ends_with("ORDER BY c.guid"));
        assert!(chars_sql(&["RUSHBF", "RUSH"]).unwrap().contains("IN ('RUSHBF', 'RUSH')"));
        assert!(chars_sql(&["x'; DROP"]).is_err() && chars_sql(&[]).is_err());
        let rows = "Tamsin\t4\t11\t1\t12\t3600\t1762400000\nbad\t1\t1\t0\t1\t0\t0\nBrakka\t2\t1\t0\t60\t0\t0\nOdo\tx\t1\t0\t1\t0\t0\n";
        let v = parse_chars(rows);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].name.as_str(), v[0].level, v[0].female, v[0].race_label.as_str(), v[0].class_label.as_str(), v[0].side.as_str()),
            ("Tamsin", 12, true, "Night Elf", "Druid", "alliance"));
        assert_eq!((v[1].name.as_str(), v[1].level, v[1].side.as_str()), ("Brakka", 60, "horde"));
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("state").join("characters.json");
        write_kept(&f, &Kept { at: 5, chars: v.clone() }).unwrap();
        assert_eq!(read_kept(&f).chars, v);
        assert_eq!(read_kept(&d.path().join("none.json")), Kept::default());
    }

    #[test]
    fn online_characters_are_the_game_folders_own_settings_folders() {
        let d = tempfile::tempdir().unwrap();
        for p in ["WTF/Account/HKABC/Wrath Unleashed/Tamsin", "WTF/Account/HKABC/Wrath Unleashed/Brakka", "WTF/Account/HKABC/Wrath Unleashed/not a name",
            "WTF/Account/SOLO/Single player/Odo", "WTF/Account/HKXYZ/wrath unleashed/Tamsin"] {
            std::fs::create_dir_all(d.path().join(p)).unwrap();
        }
        std::fs::write(d.path().join("WTF/Account/HKABC/Wrath Unleashed/Isra"), "a file, not a folder").unwrap();
        let v = online_chars(d.path(), &["Wrath Unleashed"]);
        let names: Vec<&str> = v.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Brakka", "Tamsin"], "once each, only that realm, only folders of a name's shape");
        assert!(online_chars(&d.path().join("missing"), &["Wrath Unleashed"]).is_empty());
    }
}
