//! Race and class choices at character creation on the official realm (docs/RACE-CLASS-COMBOS.md). The realm takes
//! more race/class pairs than the stock game offers (mod-arac plus realm/charfix_world.sql: "modern", today's 83
//! retail pairs; or "all", every race with every class). The desktop game only offers a pair its own tables list, so
//! before PLAY on the official realm the launcher makes those tables on this PC from the player's OWN game files and
//! puts them in `Data/patch-Y.MPQ` (and `Data/<locale>/patch-<locale>-Y.MPQ`, as the hybrid client does: sources
//! disagree on whether the root or the locale patches win). Nothing is downloaded or shipped: mod-arac's own
//! Patch-A.MPQ holds the game's tables and is never used; only the pairs (numbers) and the rules below come from us.
//!
//! The three tables (wowdev wiki, 3.3.5.12340; the same three mod-arac's Patch-A.MPQ replaces):
//! * `CharBaseInfo.dbc` (2 fields of 1 byte: race, class): which classes the create screen offers each race. The
//!   missing pairs of the preset are added after the player's own rows.
//! * `SkillRaceClassInfo.dbc` (8 fields: id, skill, race mask, class mask, flags, min level, tier, cost): which skill
//!   lines a race/class has (spellbook tabs, skills). A class-bound row (its class mask names 1 to 5 of the ten
//!   classes, so not the racial and language rows, which name nine or all) gets the race bit of every new pair of
//!   one of its classes, as mod-arac's table does (it opens those rows to every race).
//! * `CharStartOutfit.dbc` (77 fields in 296 bytes: id; race, class, sex, outfit as 4 bytes; 24 item ids, 24 display
//!   ids, 24 inventory types): what the model wears on the create screen. Each new pair gets, per sex, a copy of the
//!   same class's row of a neighbouring race ([`DONORS`], the same races realm/charfix_world.sql takes the server's
//!   starting gear from, so the preview matches what the character gets), with a new id.
//!
//! Our archive is marked (`Leyline\RaceClassCombos.txt`: format version, preset, sha256 of the player's three tables)
//! and only rewritten when that changes, so PLAY doesn't rewrite it each time. It is taken out whenever the player
//! plays anywhere else, and an archive in the slot Leyline did not write is never touched. Only for 3.3.5a game
//! folders (the caller checks the build; the tables' shapes are checked here: a table of another shape is left alone).

use leyline_terrain::dbc::Dbc;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const SLOT: &str = "patch-Y.MPQ";
pub const MARK: &str = "Leyline\\RaceClassCombos.txt";
/// Bumped when what we write changes: an archive of an older format is made again.
pub const FORMAT_VERSION: u32 = 1;
/// Ours is three small tables; anything much bigger is someone else's.
const OURS_MAX: u64 = 4 * 1024 * 1024;

pub const CHAR_BASE_INFO: &str = "DBFilesClient\\CharBaseInfo.dbc";
pub const SKILL_RACE_CLASS_INFO: &str = "DBFilesClient\\SkillRaceClassInfo.dbc";
pub const CHAR_START_OUTFIT: &str = "DBFilesClient\\CharStartOutfit.dbc";
const OUTFIT_SIZE: u32 = 296;
const OUTFIT_FIELDS: u32 = 77;
const SKILL_FIELDS: u32 = 8;

/// 3.3.5a's playable races and classes. Races: 1 Human, 2 Orc, 3 Dwarf, 4 Night Elf, 5 Undead, 6 Tauren, 7 Gnome,
/// 8 Troll, 10 Blood Elf, 11 Draenei. Classes: 1 Warrior, 2 Paladin, 3 Hunter, 4 Rogue, 5 Priest, 6 Death Knight,
/// 7 Shaman, 8 Mage, 9 Warlock, 11 Druid.
pub const RACES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11];
pub const CLASSES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 11];
const ALLIANCE: [u8; 5] = [1, 3, 4, 7, 11];
const HORDE: [u8; 5] = [2, 5, 6, 8, 10];

/// Today's retail pairs among those races and classes (83; realm/modern_combos.sql, the browser client's
/// MODERN_RACE_CLASS and docs/RACE-CLASS-COMBOS.md list the same, stream/tests/test_race_class_combos.py checks it).
#[rustfmt::skip]
pub const MODERN: &[(u8, u8)] = &[
    (1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 6), (1, 8), (1, 9),
    (2, 1), (2, 3), (2, 4), (2, 5), (2, 6), (2, 7), (2, 8), (2, 9),
    (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (3, 7), (3, 8), (3, 9),
    (4, 1), (4, 3), (4, 4), (4, 5), (4, 6), (4, 8), (4, 9), (4, 11),
    (5, 1), (5, 3), (5, 4), (5, 5), (5, 6), (5, 8), (5, 9),
    (6, 1), (6, 2), (6, 3), (6, 4), (6, 5), (6, 6), (6, 7), (6, 8), (6, 9), (6, 11),
    (7, 1), (7, 3), (7, 4), (7, 5), (7, 6), (7, 8), (7, 9),
    (8, 1), (8, 3), (8, 4), (8, 5), (8, 6), (8, 7), (8, 8), (8, 9), (8, 11),
    (10, 1), (10, 2), (10, 3), (10, 4), (10, 5), (10, 6), (10, 8), (10, 9),
    (11, 1), (11, 2), (11, 3), (11, 4), (11, 5), (11, 6), (11, 7), (11, 8), (11, 9),
];

/// (race, class, the race whose outfit it wears): realm/charfix_world.sql's ov_charfix_donor, for the 21 new modern
/// pairs. Pairs not listed ("all") take the first race of the same faction, then of the other, that has the class.
#[rustfmt::skip]
pub const DONORS: &[(u8, u8, u8)] = &[
    (1, 3, 3), (2, 5, 8), (2, 8, 8), (3, 7, 2), (3, 8, 7), (3, 9, 7), (4, 8, 1), (4, 9, 1), (5, 3, 2), (6, 2, 10),
    (6, 4, 2), (6, 5, 8), (6, 8, 8), (6, 9, 2), (7, 3, 3), (7, 5, 3), (8, 9, 2), (8, 11, 6), (10, 1, 2), (11, 4, 1),
    (11, 9, 1),
];

/// The realm's preset (the network listing's `sample.combos`, stream/network.py).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// The game's own pairs only: no archive of ours.
    Off,
    Modern,
    All,
}

impl Preset {
    /// A listing's word; anything else (or nothing) is modern, the realm's default.
    pub fn parse(s: &str) -> Preset {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" => Preset::Off,
            "all" => Preset::All,
            _ => Preset::Modern,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Preset::Off => "off",
            Preset::Modern => "modern",
            Preset::All => "all",
        }
    }

    /// The pairs the realm takes.
    pub fn pairs(self) -> Vec<(u8, u8)> {
        match self {
            Preset::Off => Vec::new(),
            Preset::Modern => MODERN.to_vec(),
            Preset::All => RACES.iter().flat_map(|&r| CLASSES.iter().map(move |&c| (r, c))).collect(),
        }
    }
}

/// The official realm's preset, from the realm list as last fetched (modern when it says none).
pub fn official_preset(app_dir: &Path) -> Preset {
    crate::realms::cached(app_dir).into_iter().find(|r| r.official).map(|r| Preset::parse(&r.combos)).unwrap_or(Preset::Modern)
}

fn race_bit(r: u8) -> u32 {
    1u32 << (r - 1)
}
fn class_bit(c: u8) -> u32 {
    1u32 << (c - 1)
}
fn playable_classes() -> u32 {
    CLASSES.iter().fold(0, |m, &c| m | class_bit(c))
}

// ------------------------------------------------------------------------------------------------ the tables
/// The player's CharBaseInfo with the preset's missing pairs added: (table, the pairs added, in order).
pub fn char_base_info(stock: &[u8], want: &[(u8, u8)]) -> Result<(Vec<u8>, Vec<(u8, u8)>), String> {
    let mut t = Dbc::parse(stock).map_err(|e| format!("CharBaseInfo: {}", e.0))?;
    if t.fields != 2 || t.record_size != 2 {
        return Err(format!("CharBaseInfo has {} fields in {} bytes, not 2 in 2: left alone.", t.fields, t.record_size));
    }
    let have: Vec<(u8, u8)> = t.records.iter().map(|r| (r[0], r[1])).collect();
    let mut added = Vec::new();
    for &(r, c) in want {
        if !have.contains(&(r, c)) && !added.contains(&(r, c)) {
            t.records.push(vec![r, c]);
            added.push((r, c));
        }
    }
    Ok((t.write(), added))
}

/// The player's SkillRaceClassInfo with every class-bound row of a new pair's class open to that pair's race too.
/// Returns the table and how many rows changed.
pub fn skill_race_class_info(stock: &[u8], new: &[(u8, u8)]) -> Result<(Vec<u8>, usize), String> {
    let mut t = Dbc::parse(stock).map_err(|e| format!("SkillRaceClassInfo: {}", e.0))?;
    if !t.shaped(SKILL_FIELDS) {
        return Err(format!("SkillRaceClassInfo has {} fields, not {SKILL_FIELDS}: left alone.", t.fields));
    }
    let pc = playable_classes();
    let mut changed = 0;
    for row in t.records.iter_mut() {
        let (races, classes) = (Dbc::get(row, 2), Dbc::get(row, 3));
        let bound = (classes & pc).count_ones();
        if races == 0 || !(1..=5).contains(&bound) {
            continue;
        }
        let add = new.iter().filter(|&&(_, c)| classes & class_bit(c) != 0).fold(0u32, |m, &(r, _)| m | race_bit(r));
        if races | add != races {
            Dbc::set(row, 2, races | add);
            changed += 1;
        }
    }
    Ok((t.write(), changed))
}

/// The race whose outfit a new pair wears: [`DONORS`], else the first race of the same faction, then of the other,
/// with a row for that class and sex in `has`.
pub fn donor(race: u8, class: u8, has: impl Fn(u8) -> bool) -> Option<u8> {
    if let Some(&(_, _, d)) = DONORS.iter().find(|&&(r, c, _)| r == race && c == class) {
        if has(d) {
            return Some(d);
        }
    }
    let (own, other) = if ALLIANCE.contains(&race) { (ALLIANCE, HORDE) } else { (HORDE, ALLIANCE) };
    own.iter().chain(other.iter()).copied().find(|&d| d != race && has(d))
}

/// The player's CharStartOutfit with a row per new pair and sex (a copy of the donor's, the race changed, a new id).
/// Returns the table and how many rows were added.
pub fn char_start_outfit(stock: &[u8], new: &[(u8, u8)]) -> Result<(Vec<u8>, usize), String> {
    let mut t = Dbc::parse(stock).map_err(|e| format!("CharStartOutfit: {}", e.0))?;
    if t.fields != OUTFIT_FIELDS || t.record_size != OUTFIT_SIZE {
        return Err(format!("CharStartOutfit has {} fields in {} bytes, not {OUTFIT_FIELDS} in {OUTFIT_SIZE}: left alone.", t.fields, t.record_size));
    }
    let find = |t: &Dbc, r: u8, c: u8, s: u8| -> Option<Vec<u8>> { t.records.iter().find(|x| x[4] == r && x[5] == c && x[6] == s).cloned() };
    let mut next = t.max_id() + 1;
    let mut added = 0;
    for &(r, c) in new {
        for sex in [0u8, 1] {
            if find(&t, r, c, sex).is_some() {
                continue;
            }
            let Some(d) = donor(r, c, |d| find(&t, d, c, sex).is_some()) else { continue };
            let Some(mut row) = find(&t, d, c, sex) else { continue };
            Dbc::set(&mut row, 0, next);
            row[4] = r;
            t.records.push(row);
            next += 1;
            added += 1;
        }
    }
    Ok((t.write(), added))
}

// ------------------------------------------------------------------------------------------------ the archive
/// What goes in our archive for a preset, from the player's own three tables.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub files: Vec<(String, Vec<u8>)>,
    pub added: Vec<(u8, u8)>,
    pub mark: String,
}

/// The mark: format, preset and the player's own tables (a change in any of them makes the archive again).
pub fn mark_text(preset: Preset, stock: &[&[u8]]) -> String {
    let mut h = Sha256::new();
    for s in stock {
        h.update((s.len() as u64).to_le_bytes());
        h.update(s);
    }
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "Leyline: the race and class choices the official realm takes, made on this PC from your own game files. \
         Removed when you play elsewhere.\r\nformat {FORMAT_VERSION}\r\npreset {}\r\ntables {hex}\r\n",
        preset.name()
    )
}

/// The archive's files from the player's own tables (CharBaseInfo, SkillRaceClassInfo, CharStartOutfit). None: the
/// game already offers every pair of the preset (or it is off), so no archive is needed. The two other tables are
/// optional: one missing or of another shape is left out (the create screen still offers the pairs).
pub fn build(preset: Preset, cbi: &[u8], srci: Option<&[u8]>, cso: Option<&[u8]>) -> Result<Option<Built>, String> {
    if preset == Preset::Off {
        return Ok(None);
    }
    let (cbi_new, added) = char_base_info(cbi, &preset.pairs())?;
    if added.is_empty() {
        return Ok(None);
    }
    let mut files = vec![(CHAR_BASE_INFO.to_string(), cbi_new)];
    let mut stock: Vec<&[u8]> = vec![cbi];
    if let Some(s) = srci {
        if let Ok((b, _)) = skill_race_class_info(s, &added) {
            files.push((SKILL_RACE_CLASS_INFO.to_string(), b));
            stock.push(s);
        }
    }
    if let Some(s) = cso {
        if let Ok((b, _)) = char_start_outfit(s, &added) {
            files.push((CHAR_START_OUTFIT.to_string(), b));
            stock.push(s);
        }
    }
    let mark = mark_text(preset, &stock);
    files.push((MARK.to_string(), mark.clone().into_bytes()));
    Ok(Some(Built { files, added, mark }))
}

fn data_dir(client: &Path) -> Result<PathBuf, String> {
    crate::clients::find_ci(client, "Data").filter(|d| d.is_dir()).ok_or_else(|| "This game folder has no Data folder.".into())
}

/// Our mark in the archive at `p`, when the archive there is ours.
fn our_mark(p: &Path) -> Option<Vec<u8>> {
    let len = fs::metadata(p).map(|m| m.len()).ok()?;
    if len > OURS_MAX {
        return None;
    }
    crate::mpq::read(&fs::read(p).ok()?, MARK)
}

/// Every place our archive goes, with whether ours is there now: `Data/patch-Y.MPQ`, then
/// `Data/<locale>/patch-<locale>-Y.MPQ` per locale folder. Err when the root slot holds someone else's archive; a
/// locale slot holding someone else's is left out.
pub fn slots(client: &Path) -> Result<Vec<(PathBuf, bool)>, String> {
    let data = data_dir(client)?;
    let mut v = Vec::new();
    match crate::clients::find_ci(&data, SLOT) {
        None => v.push((data.join(SLOT), false)),
        Some(p) if our_mark(&p).is_some() => v.push((p, true)),
        Some(_) => return Err(format!("Data/{SLOT} belongs to another mod, so Leyline leaves it alone (the realm's race and class choices need that slot).")),
    }
    for loc in crate::hybrid::locales(client) {
        let Some(dir) = crate::clients::find_ci(&data, &loc) else { continue };
        let name = format!("patch-{loc}-Y.MPQ");
        match crate::clients::find_ci(&dir, &name) {
            None => v.push((dir.join(name), false)),
            Some(p) if our_mark(&p).is_some() => v.push((p, true)),
            Some(_) => {}
        }
    }
    Ok(v)
}

/// What [`prepare_client`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum Done {
    /// Written: the pairs the create screen now offers on top of the game's own.
    Written(Vec<(u8, u8)>),
    /// Ours was already there for exactly these tables and this preset.
    Unchanged,
    /// Nothing needed (preset off, or the game already offers every pair); an archive of ours from before went.
    NotNeeded,
}

/// Before PLAY on the official realm: our archive for this preset from the player's own tables (made again only when
/// the preset, the format or the player's tables changed). `preset` Off: ours taken away.
pub fn prepare_client(client: &Path, preset: Preset) -> Result<Done, String> {
    if preset == Preset::Off {
        remove_patch(client)?;
        return Ok(Done::NotNeeded);
    }
    let all = slots(client)?;
    let order = crate::host::terrain::archive_order(client);
    if order.is_empty() {
        return Err("This game folder has no game archives in Data.".into());
    }
    let read = |n: &str| crate::mpq::read_own(&order, n);
    let cbi = match read(CHAR_BASE_INFO)? {
        Some(b) => b,
        None => return Err("your game has no CharBaseInfo table".into()),
    };
    let (srci, cso) = (read(SKILL_RACE_CLASS_INFO)?, read(CHAR_START_OUTFIT)?);
    let Some(built) = build(preset, &cbi, srci.as_deref(), cso.as_deref())? else {
        remove_patch(client)?;
        return Ok(Done::NotNeeded);
    };
    if all.iter().all(|(p, ours)| *ours && our_mark(p).as_deref() == Some(built.mark.as_bytes())) {
        return Ok(Done::Unchanged);
    }
    let bytes = crate::mpq::build(&built.files)?;
    for (p, _) in &all {
        crate::settings::write_atomic(p, &bytes).map_err(|e| format!("Couldn't write {} (is the game still running?): {e}", p.display()))?;
    }
    Ok(Done::Written(built.added))
}

/// Takes our archives away (someone else's are never touched). Ok(true): one was there.
pub fn remove_patch(client: &Path) -> Result<bool, String> {
    let Ok(data) = data_dir(client) else { return Ok(false) };
    let mut places = vec![crate::clients::find_ci(&data, SLOT)];
    for loc in crate::hybrid::locales(client) {
        places.push(crate::clients::find_ci(&data, &loc).and_then(|d| crate::clients::find_ci(&d, &format!("patch-{loc}-Y.MPQ"))));
    }
    let mut any = false;
    for p in places.into_iter().flatten() {
        if our_mark(&p).is_some() {
            fs::remove_file(&p).map_err(|e| format!("Couldn't take {} away (is the game still running?): {e}", p.display()))?;
            any = true;
        }
    }
    Ok(any)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(fields: u32, size: u32, rows: &[Vec<u8>]) -> Vec<u8> {
        Dbc { fields, record_size: size, records: rows.to_vec(), strings: vec![0] }.write()
    }

    /// The stock 3.3.5a pairs (62).
    fn stock_pairs() -> Vec<(u8, u8)> {
        #[rustfmt::skip]
        let s = [
            (1, 1), (1, 2), (1, 4), (1, 5), (1, 6), (1, 8), (1, 9), (2, 1), (2, 3), (2, 4), (2, 6), (2, 7), (2, 9),
            (3, 1), (3, 2), (3, 3), (3, 4), (3, 5), (3, 6), (4, 1), (4, 3), (4, 4), (4, 5), (4, 6), (4, 11),
            (5, 1), (5, 4), (5, 5), (5, 6), (5, 8), (5, 9), (6, 1), (6, 3), (6, 6), (6, 7), (6, 11),
            (7, 1), (7, 4), (7, 6), (7, 8), (7, 9), (8, 1), (8, 3), (8, 4), (8, 5), (8, 6), (8, 7), (8, 8),
            (10, 2), (10, 3), (10, 4), (10, 5), (10, 6), (10, 8), (10, 9), (11, 1), (11, 2), (11, 3), (11, 5),
            (11, 6), (11, 7), (11, 8),
        ];
        s.to_vec()
    }

    fn cbi() -> Vec<u8> {
        table(2, 2, &stock_pairs().iter().map(|&(r, c)| vec![r, c]).collect::<Vec<_>>())
    }

    fn u32s(v: &[u32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    fn srci() -> Vec<u8> {
        table(8, 32, &[
            u32s(&[1, 45, 0x4 | 0x8 | 0x80, class_bit(3), 0, 0, 0, 0]), // Bows: hunters of three races (class-bound)
            u32s(&[2, 98, 0x44D, 1503, 160, 0, 21, 0]),                  // Common: the Alliance, nine classes (language)
            u32s(&[3, 754, 0x1, 1535, 1170, 0, 0, 0]),                   // a Human racial: every class
            u32s(&[4, 375, 0x7FFF_FFFF, class_bit(7), 0, 0, 0, 0]),      // a shaman line, every race already
        ])
    }

    fn outfit(id: u32, r: u8, c: u8, s: u8, item: u32) -> Vec<u8> {
        let mut row = vec![0u8; OUTFIT_SIZE as usize];
        Dbc::set(&mut row, 0, id);
        row[4..8].copy_from_slice(&[r, c, s, 0]);
        Dbc::set(&mut row, 2, item);
        row
    }

    fn cso() -> Vec<u8> {
        let mut rows = Vec::new();
        let mut id = 1;
        for &(r, c) in &stock_pairs() {
            for s in [0, 1] {
                rows.push(outfit(id, r, c, s, 1000 * r as u32 + c as u32));
                id += 1;
            }
        }
        table(OUTFIT_FIELDS, OUTFIT_SIZE, &rows)
    }

    #[test]
    fn the_sets_are_83_and_100_and_keep_every_stock_pair() {
        assert_eq!(MODERN.len(), 83);
        assert_eq!(Preset::All.pairs().len(), 100);
        assert!(stock_pairs().iter().all(|p| MODERN.contains(p)));
        assert!(MODERN.iter().all(|p| Preset::All.pairs().contains(p)));
        assert_eq!(DONORS.len(), 21);
        assert!(DONORS.iter().all(|&(r, c, d)| MODERN.contains(&(r, c)) && !stock_pairs().contains(&(r, c)) && stock_pairs().contains(&(d, c))));
        assert_eq!(Preset::parse(" ALL "), Preset::All);
        assert_eq!(Preset::parse(""), Preset::Modern);
        assert_eq!(Preset::parse("../x"), Preset::Modern);
        assert!(Preset::Off.pairs().is_empty());
    }

    #[test]
    fn char_base_info_gets_the_missing_pairs_only() {
        let (b, added) = char_base_info(&cbi(), MODERN).unwrap();
        assert_eq!(added.len(), 21);
        assert!(added.contains(&(6, 2)) && added.contains(&(1, 3)) && !added.contains(&(1, 1)));
        let t = Dbc::parse(&b).unwrap();
        assert_eq!(t.records.len(), 83);
        assert_eq!(&t.records[..62], &Dbc::parse(&cbi()).unwrap().records[..], "the game's own rows first, unchanged");
        // again on its own output: nothing more
        assert!(char_base_info(&b, MODERN).unwrap().1.is_empty());
        assert_eq!(char_base_info(&cbi(), &Preset::All.pairs()).unwrap().1.len(), 38);
        // another shape is left alone
        assert!(char_base_info(&table(2, 8, &[vec![0; 8]]), MODERN).is_err());
    }

    #[test]
    fn skill_rows_open_only_class_bound_rows() {
        let (b, n) = skill_race_class_info(&srci(), &[(1, 3), (3, 7)]).unwrap();
        let t = Dbc::parse(&b).unwrap();
        assert_eq!(n, 1);
        assert_eq!(Dbc::get(&t.records[0], 2), 0x4 | 0x8 | 0x80 | race_bit(1), "Bows: Human hunters too");
        assert_eq!(Dbc::get(&t.records[1], 2), 0x44D, "a language row is not class-bound");
        assert_eq!(Dbc::get(&t.records[2], 2), 0x1, "a racial row is not class-bound");
        assert_eq!(Dbc::get(&t.records[3], 2), 0x7FFF_FFFF, "already every race");
        assert!(skill_race_class_info(&table(4, 16, &[vec![0; 16]]), &[(1, 3)]).is_err());
    }

    #[test]
    fn outfits_come_from_the_donor_with_new_ids() {
        let (b, n) = char_start_outfit(&cso(), &[(6, 2), (7, 11)]).unwrap();
        assert_eq!(n, 4, "two pairs, both sexes");
        let t = Dbc::parse(&b).unwrap();
        let tp: Vec<&Vec<u8>> = t.records.iter().filter(|r| r[4] == 6 && r[5] == 2).collect();
        assert_eq!(tp.len(), 2);
        assert_eq!(Dbc::get(tp[0], 2), 10_002, "a Tauren Paladin wears the Blood Elf paladin's outfit (charfix's donor)");
        // a Gnome Druid ("all"): the first Alliance race with druids, the Night Elf
        let gd: Vec<&Vec<u8>> = t.records.iter().filter(|r| r[4] == 7 && r[5] == 11).collect();
        assert_eq!(Dbc::get(gd[0], 2), 4_011);
        let ids: Vec<u32> = t.records.iter().map(|r| Dbc::get(r, 0)).collect();
        let mut sorted = ids.clone();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len());
        assert!(ids[124..].iter().all(|&i| i > 124), "new ids after the game's");
        assert_eq!(char_start_outfit(&b, &[(6, 2)]).unwrap().1, 0, "already there");
    }

    #[test]
    fn a_donor_of_the_same_faction_first() {
        assert_eq!(donor(3, 7, |_| true), Some(2), "charfix's: an Orc shaman's outfit for a Dwarf shaman");
        assert_eq!(donor(5, 2, |d| [1, 3, 10, 11].contains(&d)), Some(10), "Undead Paladin: the Blood Elf");
        assert_eq!(donor(7, 7, |d| [2, 6, 8, 11].contains(&d)), Some(11), "Gnome Shaman: the Draenei");
        assert_eq!(donor(7, 7, |_| false), None);
    }

    #[test]
    fn build_needs_new_pairs_and_marks_its_inputs() {
        assert_eq!(build(Preset::Off, &cbi(), None, None).unwrap(), None);
        let all: Vec<Vec<u8>> = Preset::All.pairs().iter().map(|&(r, c)| vec![r, c]).collect();
        assert_eq!(build(Preset::All, &table(2, 2, &all), None, None).unwrap(), None, "the game offers every pair already");
        let b = build(Preset::Modern, &cbi(), Some(&srci()), Some(&cso())).unwrap().unwrap();
        let names: Vec<&str> = b.files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, [CHAR_BASE_INFO, SKILL_RACE_CLASS_INFO, CHAR_START_OUTFIT, MARK]);
        assert!(b.mark.contains("format 1") && b.mark.contains("preset modern"));
        let again = build(Preset::Modern, &cbi(), Some(&srci()), Some(&cso())).unwrap().unwrap();
        assert_eq!(b, again, "the same inputs, the same archive");
        let other = build(Preset::All, &cbi(), Some(&srci()), Some(&cso())).unwrap().unwrap();
        assert_ne!(b.mark, other.mark);
        // an odd outfit table is left out, the rest still goes
        let odd = build(Preset::Modern, &cbi(), Some(&srci()), Some(&table(2, 8, &[vec![0; 8]]))).unwrap().unwrap();
        assert!(!odd.files.iter().any(|(n, _)| n == CHAR_START_OUTFIT));
    }

    fn game() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data/enUS")).unwrap();
        let common = crate::mpq::build(&[(CHAR_BASE_INFO.into(), cbi()), (SKILL_RACE_CLASS_INFO.into(), srci()), (CHAR_START_OUTFIT.into(), cso())]).unwrap();
        fs::write(d.path().join("Data/common.MPQ"), common).unwrap();
        fs::write(d.path().join("Data/enUS/locale-enUS.MPQ"), crate::mpq::build(&[("x.txt".into(), b"x".to_vec())]).unwrap()).unwrap();
        d
    }

    #[test]
    fn prepare_writes_once_follows_the_preset_and_is_removable() {
        let g = game();
        let root = g.path().join("Data").join(SLOT);
        let loc = g.path().join("Data/enUS/patch-enUS-Y.MPQ");
        match prepare_client(g.path(), Preset::Modern).unwrap() {
            Done::Written(added) => assert_eq!(added.len(), 21),
            d => panic!("{d:?}"),
        }
        assert!(root.is_file() && loc.is_file(), "the root and the locale slot");
        let t = Dbc::parse(&crate::mpq::read(&fs::read(&root).unwrap(), CHAR_BASE_INFO).unwrap()).unwrap();
        assert_eq!(t.records.len(), 83);
        // made from the player's own files, never from ours: PLAY again changes nothing
        assert_eq!(prepare_client(g.path(), Preset::Modern).unwrap(), Done::Unchanged);
        match prepare_client(g.path(), Preset::All).unwrap() {
            Done::Written(added) => assert_eq!(added.len(), 38),
            d => panic!("{d:?}"),
        }
        assert_eq!(prepare_client(g.path(), Preset::Off).unwrap(), Done::NotNeeded);
        assert!(!root.exists() && !loc.exists());
        assert!(prepare_client(g.path(), Preset::Modern).is_ok());
        assert!(remove_patch(g.path()).unwrap());
        assert!(!remove_patch(g.path()).unwrap());
    }

    #[test]
    fn someone_elses_archive_is_never_touched() {
        let g = game();
        let theirs = crate::mpq::build(&[("Interface\\x.lua".into(), b"--".to_vec())]).unwrap();
        fs::write(g.path().join("Data").join(SLOT), &theirs).unwrap();
        assert!(prepare_client(g.path(), Preset::Modern).unwrap_err().contains("another mod"));
        assert!(!remove_patch(g.path()).unwrap());
        assert_eq!(fs::read(g.path().join("Data").join(SLOT)).unwrap(), theirs);
        // their locale archive stays too, ours goes in the root once the root is free
        fs::remove_file(g.path().join("Data").join(SLOT)).unwrap();
        fs::write(g.path().join("Data/enUS/patch-enUS-Y.MPQ"), &theirs).unwrap();
        assert!(matches!(prepare_client(g.path(), Preset::Modern).unwrap(), Done::Written(_)));
        assert_eq!(fs::read(g.path().join("Data/enUS/patch-enUS-Y.MPQ")).unwrap(), theirs);
        assert!(remove_patch(g.path()).unwrap());
        assert!(g.path().join("Data/enUS/patch-enUS-Y.MPQ").is_file());
    }

    #[test]
    fn a_player_who_has_every_pair_already_gets_no_archive() {
        let g = tempfile::tempdir().unwrap();
        fs::create_dir_all(g.path().join("Data")).unwrap();
        let all: Vec<Vec<u8>> = Preset::All.pairs().iter().map(|&(r, c)| vec![r, c]).collect();
        fs::write(g.path().join("Data/common.MPQ"), crate::mpq::build(&[(CHAR_BASE_INFO.into(), table(2, 2, &all))]).unwrap()).unwrap();
        assert_eq!(prepare_client(g.path(), Preset::All).unwrap(), Done::NotNeeded);
        assert!(!g.path().join("Data").join(SLOT).exists());
    }
}
