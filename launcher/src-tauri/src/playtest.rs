//! `Leyline play-test`: the first-run test's start with no launcher window (docs/FIRST-RUN-TEST.md), the official
//! realm's version of `den-play --test` (denclick.rs). What it will do, once main.rs and app.rs call it (the plan):
//!
//! 1. `--fresh`: a new, empty data folder for this run ([`crate::settings::APP_DIR_ENV`], set before anything
//!    else reads the settings), so nothing of the owner's own launcher is used or changed;
//! 2. `--game <folder>`: add that game folder, as Find your game files does (clients::detect, new_profile);
//! 3. sign in with the test Hero Key from [`KEY_ENV`] (never the command line: other programs can read that), as
//!    the Log In button's "Sign in with your Hero Key" does (social::sign_in_key);
//! 4. `--official`: PLAY on the official realm the way the PLAY button does (app.rs play_official: the website's
//!    game login, the hybrid glue signs in), with `--world` entering the world with the test character (the
//!    hybrid Session's `enter`), else stopping at character select;
//! 5. write [`REPORT`] in the data folder, close the game after `--hold` seconds, and exit 0 when it got there.
//!
//! This file is the contract (the arguments, the names the test tool reads); its parsing is tested here. main.rs sets
//! the fresh folder, app.rs `play_test_cli` does the rest; tools/first-run/first_run.py (Control: first-run-test) runs it.
//!
//! `--checks` (docs/FRESH-COMPUTER-TEST.md, owner 3 Nov: "a clean test from a fresh computer to test every feature"):
//! the launcher's own code paths for what a new player does around PLAY, each written to the report's `checks`:
//! * `drops`: what the game folder box does with each thing a player can drop on it ([`drop_probes`], judged by
//!   [`judge`]): the game folder is added, a zip unpacks, a link downloads, and a magnet link, a `.torrent` or a
//!   shortcut to a magnet are refused with one sentence while Settings › Testing is off (it is, in a fresh profile);
//! * `signin`: the Hero Key file found the way the sign-in screen finds one in Downloads, log out, then the key pasted;
//! * `realm_switch`: a community realm from the list, then back to the official one (app.rs, after PLAY);
//! * `loadouts`: Clean, then back to what was on ([`loadout_round_trip`], after the game closed), the game folder's
//!   loadouts file and its mods put back exactly as they were.
//!
//! A second `--game` (and more) adds those folders too, as a player with several versions does; PLAY uses the first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// In the launcher program, so the first-run tool can tell a launcher that has `play-test` from an older one.
pub const CLI_MARK: &str = "leyline-play-test-v1";
/// The test account's Hero Key, from the first-run tool (tools/audit's test account), only in the environment.
pub const KEY_ENV: &str = "LEYLINE_TEST_HEROKEY";
/// `<data folder>/play-test.json`: the run's report (the first-run tool reads it).
pub const REPORT: &str = "play-test.json";
/// The longest `--hold`: the game stays open this many seconds at most after it got there.
pub const HOLD_MAX: u64 = 600;
/// In the launcher program beside [`CLI_MARK`] when it can do `--checks` (the first-run tool asks for them only then).
pub const CHECKS_MARK: &str = "leyline-play-test-checks-v1";
/// The folder in the (fresh) data folder where the drop probes and the test key file are made; removed after.
pub const PROBE_DIR: &str = "fresh-test-drops";
/// More game folders than this on one command line is a mistake.
pub const MAX_GAMES: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct Cli {
    /// PLAY on the official realm (today the only realm the test plays).
    pub official: bool,
    /// A new, empty data folder for this run.
    pub fresh: bool,
    /// The game folder to add (a fresh data folder has none); PLAY uses it.
    pub game: Option<PathBuf>,
    /// More game folders to add (other versions), not played.
    pub more: Vec<PathBuf>,
    /// The feature checks around PLAY (see the module's words).
    pub checks: bool,
    /// Enter the world, not only character select.
    pub world: bool,
    /// Seconds the game stays open after it got there (for the screenshot), then it is closed.
    pub hold: u64,
    /// `--single <single|vanilla>`: single player's playtest on that local world instead of the official realm
    /// (crate::sptest; app.rs single_play_test_cli).
    pub single: Option<String>,
    /// `--checks <list>` with `--single`: which of its checks run (`--checks` alone: every one).
    pub sp_checks: Vec<crate::sptest::Check>,
    /// `--level <n>` with `--single`: the level the levelup check goes to.
    pub level_to: u8,
    /// `--bossrush`: Boss Rush's playtest (start anywhere and a full boss fight; crate::sptest Check::BossStart /
    /// BossFight; app.rs bossrush_play_test_cli). Its checks are in `sp_checks`.
    pub bossrush: bool,
    /// `--boss <start>`: the boss, wing or whole raid it starts at (host/bossrush.rs); the final fight by default.
    pub boss: String,
    /// `--size <10|25>` and `--diff <normal|heroic>` with `--bossrush`.
    pub br_size: u8,
    pub br_heroic: bool,
    /// `--hero <class>-<spec>`: the hero the test plays (on the bots' own AI during the fight).
    pub hero: String,
    /// `--fight-max <s>`: how long the fight is watched at most.
    pub fight_max: u64,
}

impl Default for Cli {
    fn default() -> Self {
        Cli {
            official: false,
            fresh: false,
            game: None,
            more: Vec::new(),
            checks: false,
            world: false,
            hold: 30,
            single: None,
            sp_checks: Vec::new(),
            level_to: crate::sptest::LEVEL_TO,
            bossrush: false,
            boss: "frozen-throne".into(),
            br_size: 10,
            br_heroic: false,
            hero: "mage-fire".into(),
            fight_max: crate::sptest::FIGHT_MAX_S,
        }
    }
}

/// `Leyline play-test [--official] [--fresh] [--game <folder>]... [--checks] [--world] [--hold <s>]`, or single
/// player's: `Leyline play-test --single <single|vanilla> [--fresh] [--game <folder>] [--hold <s>] [--level <n>]
/// [--checks levelup,companions,stoponexit,repair]` (crate::sptest). None when the first argument isn't `play-test`;
/// Err for an argument it doesn't know (a key or a password on the command line among them: those go through
/// [`KEY_ENV`] only).
pub fn parse_cli(args: &[String]) -> Option<Result<Cli, String>> {
    if args.get(1).map(String::as_str) != Some("play-test") {
        return None;
    }
    let mut c = Cli::default();
    let mut list: Option<String> = None;
    let mut it = args.iter().skip(2).peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--official" => c.official = true,
            "--fresh" => c.fresh = true,
            "--world" => c.world = true,
            "--checks" => {
                c.checks = true;
                // a list after it (single player's checks); the next flag isn't one
                if let Some(l) = it.next_if(|n| !n.starts_with("--")) {
                    list = Some(l.clone());
                }
            }
            "--single" => match it.next().map(|w| w.trim().to_ascii_lowercase()) {
                Some(w) if w == "single" || w == "vanilla" => c.single = Some(w),
                _ => return Some(Err("--single takes single or vanilla".into())),
            },
            "--level" => match it.next().and_then(|n| n.parse::<u8>().ok()) {
                Some(n) if (2..=80).contains(&n) => c.level_to = n,
                _ => return Some(Err("--level takes a level from 2 to 80".into())),
            },
            "--game" => match it.next() {
                Some(g) if !g.trim().is_empty() => {
                    let g = PathBuf::from(g.trim());
                    if c.game.is_none() {
                        c.game = Some(g);
                    } else if c.more.len() + 1 >= MAX_GAMES {
                        return Some(Err(format!("play-test takes at most {MAX_GAMES} game folders")));
                    } else if c.game.as_ref() != Some(&g) && !c.more.contains(&g) {
                        c.more.push(g);
                    }
                }
                _ => return Some(Err("--game needs the game folder".into())),
            },
            "--hold" => c.hold = it.next().and_then(|h| h.parse().ok()).unwrap_or(30).min(HOLD_MAX),
            "--bossrush" => c.bossrush = true,
            "--boss" => match it.next() {
                Some(b) if crate::host::bossrush::start(b).is_some() => c.boss = b.clone(),
                _ => return Some(Err("--boss takes a boss, a wing or a whole raid of Boss Rush (frozen-throne, icc-plague, icc-raid...)".into())),
            },
            "--size" => match it.next().map(String::as_str) {
                Some("10") => c.br_size = 10,
                Some("25") => c.br_size = 25,
                _ => return Some(Err("--size takes 10 or 25".into())),
            },
            "--diff" => match it.next().map(String::as_str) {
                Some("normal") => c.br_heroic = false,
                Some("heroic") => c.br_heroic = true,
                _ => return Some(Err("--diff takes normal or heroic".into())),
            },
            "--hero" => match it.next() {
                Some(h) if h.split_once('-').is_some_and(|(cl, sp)| crate::host::bossrush::check("frozen-throne", "normal", cl, sp, "exact").is_ok()) => c.hero = h.clone(),
                _ => return Some(Err("--hero takes <class>-<spec>, e.g. mage-fire or paladin-holy".into())),
            },
            "--fight-max" => match it.next().and_then(|n| n.parse::<u64>().ok()) {
                Some(n) if (60..=3600).contains(&n) => c.fight_max = n,
                _ => return Some(Err("--fight-max takes 60 to 3600 seconds".into())),
            },
            other => return Some(Err(format!("play-test doesn't take {other:?}"))),
        }
    }
    if c.bossrush {
        if c.official || c.single.is_some() {
            return Some(Err("play-test plays one thing: --bossrush, --single or --official".into()));
        }
        c.sp_checks = match list.as_deref() {
            Some(l) => match crate::sptest::parse_boss_checks(l) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            },
            None => crate::sptest::BOSS_ALL.to_vec(),
        };
        if c.sp_checks.contains(&crate::sptest::Check::BossFight) && crate::host::bossrush::start(&c.boss).map(|s| s.0) == Some(crate::host::bossrush::Scope::Raid) {
            return Some(Err("bossfight fights one boss: pick a boss or a wing with --boss".into()));
        }
        return Some(Ok(c));
    }
    if c.single.is_some() {
        if c.official {
            return Some(Err("play-test plays the official realm or a single player world, not both".into()));
        }
        c.sp_checks = match list.as_deref() {
            Some(l) => match crate::sptest::parse_checks(l) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            },
            None if c.checks => crate::sptest::ALL.to_vec(),
            None => Vec::new(),
        };
        return Some(Ok(c));
    }
    if list.is_some() {
        return Some(Err("a list of checks goes with --single".into()));
    }
    if !c.official {
        return Some(Err("play-test plays the official realm: add --official (or --single <single|vanilla>)".into()));
    }
    Some(Ok(c))
}

/// The fresh data folder for a run started at `now_ms`, under `base` (the system's temp folder).
pub fn fresh_dir(base: &Path, now_ms: u64) -> PathBuf {
    base.join(format!("leyline-first-run-{now_ms}"))
}

// ------------------------------------------------------------------------------------------------ the drop probes

/// What the game folder box should make of a dropped thing (clientimport::classify_with).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Expect {
    /// Added where it is.
    Folder,
    /// Unpacked into a folder the player picks.
    Zip,
    /// Downloaded (the player's own backup), then unpacked.
    Link,
    /// `clientimport::TORRENT_REFUSAL`: Leyline doesn't move game clients by torrent (Settings › Testing off).
    Refused,
    /// Another archive: "zip files only".
    NotAZip,
}

/// One thing to drop: a name for the report, what the drop hands the launcher (a path or a text), what should happen.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Probe {
    pub name: &'static str,
    pub input: String,
    pub expect: Expect,
}

/// A made-up info-hash: names no torrent anyone shares.
const PROBE_BTIH: &str = "0123456789abcdef0123456789abcdef01234567";

/// The things a player can drop on the game folder box, the small files among them made in `dir` (a zip's first
/// bytes, an empty `.torrent`, a browser's shortcut to a magnet link, a 7z's name: nothing of a game). `game`: the
/// player's own game folder (dropped as a folder, and as a browser hands a folder: a `file://` address).
pub fn drop_probes(dir: &Path, game: Option<&Path>) -> std::io::Result<Vec<Probe>> {
    std::fs::create_dir_all(dir)?;
    let mut zip = b"PK\x03\x04".to_vec();
    zip.extend_from_slice(&[0u8; 60]);
    std::fs::write(dir.join("my-backup.zip"), &zip)?;
    std::fs::write(dir.join("a-client.torrent"), b"d4:infod4:name4:testee")?;
    std::fs::write(dir.join("a-link.url"), format!("[InternetShortcut]\r\nURL=magnet:?xt=urn:btih:{PROBE_BTIH}&dn=test\r\n"))?;
    std::fs::write(dir.join("my-backup.7z"), b"7z\xBC\xAF\x27\x1C")?;
    let mut out = Vec::new();
    if let Some(g) = game {
        out.push(Probe { name: "your game folder", input: g.display().to_string(), expect: Expect::Folder });
    }
    let p = |n: &str| dir.join(n).display().to_string();
    out.push(Probe { name: "a zip of your game folder", input: p("my-backup.zip"), expect: Expect::Zip });
    out.push(Probe { name: "a link to your own backup", input: "https://example.com/my-backup.zip".into(), expect: Expect::Link });
    out.push(Probe { name: "a magnet link", input: format!("magnet:?xt=urn:btih:{PROBE_BTIH}&dn=test"), expect: Expect::Refused });
    out.push(Probe { name: "a .torrent file", input: p("a-client.torrent"), expect: Expect::Refused });
    out.push(Probe { name: "a link to a .torrent", input: "https://example.com/a-client.torrent".into(), expect: Expect::Refused });
    out.push(Probe { name: "a browser shortcut to a magnet link", input: p("a-link.url"), expect: Expect::Refused });
    out.push(Probe { name: "a 7z archive", input: p("my-backup.7z"), expect: Expect::NotAZip });
    Ok(out)
}

/// Whether the launcher did what the probe expects. `torrents_on`: Settings › Testing's switch (a fresh profile: off).
pub fn judge(expect: Expect, got: &Result<crate::clientimport::Source, String>, torrents_on: bool) -> bool {
    use crate::clientimport::Source;
    match (expect, got) {
        (Expect::Folder, Ok(Source::Folder { .. })) => true,
        (Expect::Zip, Ok(Source::Zip { .. })) => true,
        (Expect::Link, Ok(Source::Link { .. })) => true,
        (Expect::Refused, Err(e)) => !torrents_on && e == crate::clientimport::TORRENT_REFUSAL,
        (Expect::NotAZip, Err(e)) => e.contains("zip files only"),
        _ => false,
    }
}

/// What a probe came to, for the report: the kind, or the sentence the player sees.
pub fn got_text(got: &Result<crate::clientimport::Source, String>) -> String {
    use crate::clientimport::Source;
    match got {
        Ok(Source::Folder { .. }) => "folder".into(),
        Ok(Source::Zip { .. }) => "zip".into(),
        Ok(Source::Link { .. }) => "link".into(),
        Ok(Source::Magnet { .. }) => "magnet (taken: the testing switch is on)".into(),
        Ok(Source::Torrent { .. }) => "torrent (taken: the testing switch is on)".into(),
        Ok(Source::TorrentLink { .. }) => "torrent link (taken: the testing switch is on)".into(),
        Err(e) => e.clone(),
    }
}

/// Every probe dropped through the launcher's own reading of a drop: `[{name, expect, got, ok}]`.
pub fn run_drops(probes: &[Probe], torrents_on: bool) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = probes
        .iter()
        .map(|p| {
            let got = crate::clientimport::classify_with(&p.input, torrents_on);
            serde_json::json!({ "name": p.name, "expect": p.expect, "got": got_text(&got), "ok": judge(p.expect, &got, torrents_on) })
        })
        .collect();
    serde_json::Value::Array(rows)
}

// ------------------------------------------------------------------------------------------------ loadouts

fn sorted_on(client: &Path) -> Vec<String> {
    let mut v = crate::loadouts::enabled_now(&crate::mods::load(client));
    v.sort();
    v
}

fn dir_names(d: &Path) -> HashSet<String> {
    std::fs::read_dir(d).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default()
}

/// Loadouts switch and back on the player's game folder: a loadout of what is on now, Clean (every Leyline mod off),
/// then that loadout again. No graphics are written. After, always: the folder's loadouts file as it was, the WTF
/// copies this made removed, and every Leyline mod on or off as it was (a safety net past the loadout itself).
pub fn loadout_round_trip(client: &Path) -> serde_json::Value {
    use crate::loadouts;
    let book = loadouts::path(client);
    let book_before = std::fs::read(&book).ok();
    let backups = client.join(crate::mods::DIR).join(loadouts::BACKUP_DIR);
    let backups_existed = backups.is_dir();
    let had = dir_names(&backups);
    let before = sorted_on(client);
    let mut out = serde_json::json!({ "mods_on": before.len() });
    match loadouts::add(client, "Fresh computer test", Some(before.clone()), "", "fresh-test") {
        Err(e) => {
            out["ok"] = serde_json::Value::Null;
            out["why"] = format!("no test loadout: {e}").into();
        }
        Ok(l) => {
            let clean = loadouts::switch(client, loadouts::CLEAN, None);
            let on_clean = sorted_on(client);
            let back = loadouts::switch(client, &l.id, None);
            let after = sorted_on(client);
            let clean_ok = clean.as_ref().is_ok_and(|s| s.problems.is_empty()) && on_clean.is_empty();
            let back_ok = back.as_ref().is_ok_and(|s| s.problems.is_empty()) && after == before;
            out["clean"] = serde_json::json!({ "ok": clean_ok, "still_on": on_clean, "why": clean.err().unwrap_or_default() });
            out["back"] = serde_json::json!({ "ok": back_ok, "on": after.len(), "why": back.err().unwrap_or_default() });
            out["ok"] = (clean_ok && back_ok).into();
        }
    }
    // put back, always
    let mut put_back = true;
    let now = sorted_on(client);
    for k in before.iter().filter(|k| !now.contains(k)) {
        put_back &= crate::mods::set_enabled(client, k, true).is_ok();
    }
    for k in now.iter().filter(|k| !before.contains(k)) {
        put_back &= crate::mods::set_enabled(client, k, false).is_ok();
    }
    match &book_before {
        Some(b) => put_back &= crate::settings::write_atomic(&book, b).is_ok(),
        None => {
            if book.exists() {
                put_back &= std::fs::remove_file(&book).is_ok();
            }
        }
    }
    for n in dir_names(&backups).difference(&had) {
        put_back &= std::fs::remove_dir_all(backups.join(n)).is_ok();
    }
    if !backups_existed && backups.is_dir() && dir_names(&backups).is_empty() {
        let _ = std::fs::remove_dir(&backups);
    }
    put_back &= sorted_on(client) == before && std::fs::read(&book).ok() == book_before;
    out["put_back"] = put_back.into();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        std::iter::once("Leyline").chain(a.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn only_play_test() {
        assert_eq!(parse_cli(&args(&["den-play", "--test"])), None);
        assert_eq!(parse_cli(&args(&[])), None);
    }

    #[test]
    fn the_first_run_line() {
        let c = parse_cli(&args(&[
            "play-test",
            "--official",
            "--fresh",
            "--game",
            "C:\\Games\\Wrath",
            "--world",
            "--hold",
            "45",
        ]))
        .unwrap()
        .unwrap();
        assert!(c.official && c.fresh && c.world);
        assert_eq!(c.game, Some(PathBuf::from("C:\\Games\\Wrath")));
        assert_eq!(c.hold, 45);
        let d = parse_cli(&args(&["play-test", "--official"])).unwrap().unwrap();
        assert!(!d.fresh && !d.world && d.game.is_none() && d.hold == 30);
        assert_eq!(parse_cli(&args(&["play-test", "--official", "--hold", "99999"])).unwrap().unwrap().hold, HOLD_MAX);
    }

    #[test]
    fn refused() {
        assert!(parse_cli(&args(&["play-test"])).unwrap().is_err(), "the official realm only, said");
        assert!(
            parse_cli(&args(&["play-test", "--official", "--key", "ovk1.x"])).unwrap().is_err(),
            "no key on the command line"
        );
        assert!(parse_cli(&args(&["play-test", "--official", "--password", "x"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--official", "--game"])).unwrap().is_err());
    }

    #[test]
    fn several_game_folders_and_checks() {
        let c = parse_cli(&args(&["play-test", "--official", "--checks", "--game", "C:\\W", "--game", "C:\\V", "--game", "C:\\W"]))
            .unwrap()
            .unwrap();
        assert!(c.checks);
        assert_eq!(c.game, Some(PathBuf::from("C:\\W")), "the first is the one PLAY uses");
        assert_eq!(c.more, vec![PathBuf::from("C:\\V")], "the same folder twice is one");
        assert!(!parse_cli(&args(&["play-test", "--official"])).unwrap().unwrap().checks);
        let names: Vec<String> = (0..=MAX_GAMES).map(|i| format!("C:\\G{i}")).collect();
        let mut many: Vec<&str> = vec!["play-test", "--official"];
        for n in &names {
            many.push("--game");
            many.push(n.as_str());
        }
        assert!(parse_cli(&args(&many)).unwrap().is_err());
    }

    #[test]
    fn single_player_line() {
        use crate::sptest::Check;
        let c = parse_cli(&args(&[
            "play-test", "--fresh", "--game", "C:\\Games\\Wrath", "--single", "single", "--hold", "20", "--checks",
            "levelup,companions,stoponexit,repair",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(c.single.as_deref(), Some("single"));
        assert!(c.fresh && !c.official);
        assert_eq!(c.sp_checks, vec![Check::Companions, Check::Levelup, Check::StopOnExit, Check::Repair]);
        assert_eq!(c.level_to, crate::sptest::LEVEL_TO);
        assert_eq!(c.hold, 20);
        // --checks alone: every check; then another flag right after it isn't taken as the list
        let all = parse_cli(&args(&["play-test", "--single", "VANILLA", "--checks", "--level", "12"])).unwrap().unwrap();
        assert_eq!(all.single.as_deref(), Some("vanilla"));
        assert_eq!(all.sp_checks, crate::sptest::ALL.to_vec());
        assert_eq!(all.level_to, 12);
        let none = parse_cli(&args(&["play-test", "--single", "single"])).unwrap().unwrap();
        assert!(none.sp_checks.is_empty());
        assert!(parse_cli(&args(&["play-test", "--single", "bossrush"])).unwrap().is_err(), "Boss Rush has its own page");
        assert!(parse_cli(&args(&["play-test", "--single"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--single", "single", "--official"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--single", "single", "--checks", "levelup,fly"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--single", "single", "--level", "1"])).unwrap().is_err());
    }

    #[test]
    fn boss_rush_line() {
        use crate::sptest::Check;
        let c = parse_cli(&args(&["play-test", "--fresh", "--game", "C:\\W", "--bossrush", "--checks", "bossstart,bossfight", "--hold", "20"])).unwrap().unwrap();
        assert!(c.bossrush && c.fresh && c.single.is_none());
        assert_eq!((c.boss.as_str(), c.br_size, c.br_heroic, c.hero.as_str()), ("frozen-throne", 10, false, "mage-fire"));
        assert_eq!(c.sp_checks, vec![Check::BossStart, Check::BossFight]);
        let d = parse_cli(&args(&["play-test", "--bossrush", "--boss", "icc-plague", "--size", "25", "--diff", "heroic", "--hero", "paladin-holy", "--fight-max", "900"])).unwrap().unwrap();
        assert_eq!((d.boss.as_str(), d.br_size, d.br_heroic, d.hero.as_str(), d.fight_max), ("icc-plague", 25, true, "paladin-holy", 900));
        assert_eq!(d.sp_checks, crate::sptest::BOSS_ALL.to_vec(), "both checks by default");
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--boss", "nope"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--size", "20"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--hero", "mage-bear"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--checks", "levelup"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--official"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--boss", "icc-raid", "--checks", "bossfight"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--bossrush", "--boss", "icc-raid", "--checks", "bossstart"])).unwrap().is_ok());
        assert!(parse_cli(&args(&["play-test", "--single", "single", "--level", "x"])).unwrap().is_err());
        assert!(parse_cli(&args(&["play-test", "--official", "--checks", "levelup"])).unwrap().is_err(), "the list is single player's");
        assert!(parse_cli(&args(&["play-test", "--single", "single", "--password", "x"])).unwrap().is_err());
    }

    #[test]
    fn drops_as_the_game_folder_box_takes_them() {
        let d = tempfile::tempdir().unwrap();
        let game = d.path().join("My Game");
        std::fs::create_dir_all(&game).unwrap();
        let probes = drop_probes(&d.path().join(PROBE_DIR), Some(&game)).unwrap();
        assert_eq!(probes.len(), 8);
        assert_eq!(probes.iter().filter(|p| p.expect == Expect::Refused).count(), 4, "magnet, .torrent, its link, a shortcut to a magnet");
        // a fresh profile: Settings › Testing off, so every torrent way is refused with the one sentence
        let rows = run_drops(&probes, false);
        for r in rows.as_array().unwrap() {
            assert_eq!(r["ok"], true, "{r}");
        }
        assert!(rows.to_string().contains(crate::clientimport::TORRENT_REFUSAL));
        // the owner's testing switch on: the torrent ways are taken, so the fresh-computer line says so (FAIL)
        let on = run_drops(&probes, true);
        let taken: Vec<&serde_json::Value> = on.as_array().unwrap().iter().filter(|r| r["ok"] == false).collect();
        assert!(taken.len() >= 3, "{on}");
        assert!(taken.iter().all(|r| r["expect"] == "refused"));
        // without a game folder the folder probe is left out
        assert_eq!(drop_probes(&d.path().join("again"), None).unwrap().len(), 7);
    }

    #[test]
    fn judge_is_strict() {
        use crate::clientimport::Source;
        let folder: Result<Source, String> = Ok(Source::Folder { path: PathBuf::from("/x") });
        assert!(judge(Expect::Folder, &folder, false));
        assert!(!judge(Expect::Zip, &folder, false));
        let other: Result<Source, String> = Err("Leyline can't find that file or folder.".into());
        assert!(!judge(Expect::Refused, &other, false), "a refusal is the torrent sentence, not any error");
        let refused: Result<Source, String> = Err(crate::clientimport::TORRENT_REFUSAL.into());
        assert!(judge(Expect::Refused, &refused, false));
        assert!(!judge(Expect::Refused, &refused, true));
    }

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    /// A game folder with two Leyline mods on, one off, an addon the player put in, and WTF.
    fn mod_folder() -> tempfile::TempDir {
        use crate::mods::{Installed, Manifest};
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "Interface/AddOns/Mine/Mine.toc", "## Interface: 30300\n");
        put(r, "WTF/Config.wtf", "SET farclip \"777\"\n");
        let mut man = Manifest { schema: 1, build: "3.3.5a".into(), ..Default::default() };
        for (key, addon) in [("up:skada", "Skada"), ("up:dominos", "Dominos")] {
            let f = format!("Interface/AddOns/{addon}/{addon}.toc");
            put(r, &f, "## Interface: 30300\n");
            man.mods.push(Installed {
                key: key.into(),
                name: addon.into(),
                source: "catalog".into(),
                enabled: true,
                addons: vec![addon.into()],
                files: vec![f],
                dirs: vec![format!("Interface/AddOns/{addon}")],
                ..Default::default()
            });
        }
        put(r, "Leyline/mods.json", &serde_json::to_string(&man).unwrap());
        d
    }

    #[test]
    fn loadouts_clean_and_back_leave_the_folder_as_it_was() {
        let d = mod_folder();
        let r = d.path();
        let before = sorted_on(r);
        assert_eq!(before.len(), 2);
        let out = loadout_round_trip(r);
        assert_eq!(out["ok"], true, "{out}");
        assert_eq!(out["clean"]["ok"], true, "{out}");
        assert_eq!(out["back"]["ok"], true, "{out}");
        assert_eq!(out["put_back"], true, "{out}");
        assert_eq!(sorted_on(r), before);
        assert!(!crate::loadouts::path(r).exists(), "there was no loadouts file before: none after");
        assert!(!r.join("Leyline").join(crate::loadouts::BACKUP_DIR).exists(), "the WTF copies it made are gone");
        assert!(r.join("Interface/AddOns/Mine/Mine.toc").is_file(), "the player's own addon untouched");
        assert!(r.join("Interface/AddOns/Skada/Skada.toc").is_file());
        // a loadouts file that was there comes back byte for byte
        crate::loadouts::add(r, "Raider", None, "", "").unwrap();
        let book = std::fs::read(crate::loadouts::path(r)).unwrap();
        let out = loadout_round_trip(r);
        assert_eq!(out["put_back"], true, "{out}");
        assert_eq!(std::fs::read(crate::loadouts::path(r)).unwrap(), book);
    }

    #[test]
    fn fresh_dir_is_new_per_run() {
        let base = Path::new("/tmp");
        assert_ne!(fresh_dir(base, 1), fresh_dir(base, 2));
        assert!(fresh_dir(base, 1).starts_with(base));
    }
}
