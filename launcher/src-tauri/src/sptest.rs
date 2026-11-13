//! `Leyline play-test --single <single|vanilla>`: single player's self-driving playtest on the owner's own PC (owner
//! 11 Nov: "make sure you do your own playtests on my pc and level up before releasing and then release"). It runs
//! the one-click flow through the same Rust functions the Single player page uses (app.rs `single_play_test_cli`:
//! world_find_folder, world_preflight, world_setup, world_play with `first`), waits for the character to be in the
//! world, then the checks:
//!
//! * `companions`: the bots that join by themselves (ov_companions.lua "single player") are in the character's group
//!   and online within [`COMPANIONS_WITHIN_S`] seconds, as many as the world's settings say ([`expected_companions`]);
//! * `levelup`: **server-driven levelling** on the test's own local world only (SOAP on 127.0.0.1, the world's own GM
//!   login): `character level <name> <next>` step by step to `--level` (10), each step's level read back from the database after
//!   `.saveall`, then the companions following the player's level. Labelled as such in the report: not hand play;
//! * `stoponexit`: the game the test started closed, the world stopped by itself (Settings › Stop my world when the
//!   game closes, on by default) within [`STOP_WITHIN_S`] seconds;
//! * `repair`: one non-critical engine file deleted ([`pick_victim`]), Repair, the file back, the world up, Play again
//!   into the world.
//!
//! This file holds the pure parts (the checks' names, the SQL, reading the rows, the report); app.rs drives them.
//! tools/first-run/sp_playtest.py (Control: `sp-playtest`) runs it on a fresh data folder and takes the screenshots at
//! the report's phases. Nothing here prints or keeps a password or a Hero Key.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// In the launcher program when it has the single player playtest (the test tool checks the download for it).
pub const SP_MARK: &str = "leyline-play-test-single-v1";
/// `<data folder>/sp-play-test.json`: the run's report.
pub const REPORT: &str = "sp-play-test.json";
/// `<data folder>/sp-play-test.log`: every progress line (also on stdout).
pub const LOG: &str = "sp-play-test.log";
/// The companions must be in the group within this many seconds of entering the world.
pub const COMPANIONS_WITHIN_S: u64 = 30;
/// How long the companions are watched for at most (a late arrival is reported with its time, as a FAIL).
pub const COMPANIONS_WATCH_S: u64 = 120;
/// After the game closes: the watcher's 60 s, the save and the stop.
pub const STOP_WITHIN_S: u64 = 150;
/// From Play to the character in the world (the game starting, the login, Quick start, the loading screen).
pub const IN_WORLD_S: u64 = 480;
/// The level `levelup` goes to unless `--level` says otherwise.
pub const LEVEL_TO: u8 = 10;
/// How the levelling is labelled in the report (honest: the server gave the levels, nobody played them).
pub const LEVEL_NOTE: &str = "server-driven levelling: GM character level on the test's own local world (127.0.0.1), not hand play";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    Companions,
    Levelup,
    StopOnExit,
    Repair,
    /// Boss Rush (`play-test --bossrush`): the one click puts the hero in front of the named boss with a full
    /// companion raid ([`judge_start`]).
    BossStart,
    /// Boss Rush: the boss fought to an end by the companion raid (the hero on the bots' own AI), its start, its
    /// phases and its outcome recorded, then Retry back in front of the boss ([`judge_fight`]).
    BossFight,
}

/// Every check, in the order they run.
pub const ALL: [Check; 4] = [Check::Companions, Check::Levelup, Check::StopOnExit, Check::Repair];

impl Check {
    pub fn word(self) -> &'static str {
        match self {
            Check::Companions => "companions",
            Check::Levelup => "levelup",
            Check::StopOnExit => "stoponexit",
            Check::Repair => "repair",
            Check::BossStart => "bossstart",
            Check::BossFight => "bossfight",
        }
    }
}

/// Boss Rush's checks, in the order they run.
pub const BOSS_ALL: [Check; 2] = [Check::BossStart, Check::BossFight];

/// `bossstart,bossfight` (`all` for both) into the run order.
pub fn parse_boss_checks(text: &str) -> Result<Vec<Check>, String> {
    let mut want = Vec::new();
    for w in text.split(',').map(|w| w.trim().to_ascii_lowercase()).filter(|w| !w.is_empty()) {
        match w.as_str() {
            "all" => want.extend(BOSS_ALL),
            other => match BOSS_ALL.iter().find(|c| c.word() == other) {
                Some(c) => want.push(*c),
                None => return Err(format!("--checks with --bossrush doesn't know {other:?} (bossstart, bossfight, all)")),
            },
        }
    }
    if want.is_empty() {
        return Err("--checks needs at least one check".into());
    }
    Ok(BOSS_ALL.into_iter().filter(|c| want.contains(c)).collect())
}

// ------------------------------------------------------------------------------------------------ Boss Rush's checks

/// In the launcher program when it has the Boss Rush playtest (the test tool checks the download for it).
pub const BR_MARK: &str = "leyline-play-test-bossrush-v1";
/// `<data folder>/br-play-test.json`: the Boss Rush run's report.
pub const BR_REPORT: &str = "br-play-test.json";
/// From the click to the raid standing in front of the boss: the game, the login, the hero, the bots (the module waits
/// up to 60 s for them on Boss Rush's profile).
pub const START_WITHIN_S: u64 = 420;
/// The hero stands this close to the start (yards): the raid lands in rows of three, a few yards apart.
pub const NEAR_YD: f32 = 15.0;
/// From the pull to the boss engaged (the final fight's paladin walks up and talks first).
pub const ENGAGE_WITHIN_S: u64 = 240;
/// The longest fight watched unless `--fight-max` says otherwise (the final fight runs 10 to 15 minutes cleared).
pub const FIGHT_MAX_S: u64 = 1500;
/// After a wipe or a clear, Retry has the raid in front of the boss again within this.
pub const RETRY_WITHIN_S: u64 = 240;

/// Is the raid in front of its start, complete: the start's map, an instance of its own, ready (not fighting), the
/// hero within [`NEAR_YD`] of the start, `size` members of whom `size - 1` companions, everyone inside.
pub fn judge_start(r: &crate::host::bossrush::RushRun, start: &str, map: u32, size: u32) -> Result<String, String> {
    let line = format!(
        "{} at {} ({}), map {} instance {}, {:.1} yd from the start, {} in the raid ({} companions, {} inside), phase {}",
        r.leader, if r.step.is_empty() { "-" } else { r.step.as_str() }, r.diff, r.map, r.instance, r.dist, r.members, r.bots, r.inside, r.phase
    );
    if r.start != start {
        return Err(format!("the run is {} not {start}: {line}", r.start));
    }
    if r.map != map || r.instance == 0 {
        return Err(format!("not in the raid's own instance: {line}"));
    }
    if r.phase != "ready" {
        return Err(format!("not ready: {line}"));
    }
    if r.dist > NEAR_YD {
        return Err(format!("not in front of the boss: {line}"));
    }
    if r.members != size || r.bots + 1 != size || r.inside != size {
        return Err(format!("the raid isn't complete: {line}"));
    }
    Ok(line)
}

/// What happened between two looks at the run: the phase changing, the boss's phase casts, bosses down, results.
pub fn fight_events(prev: Option<&crate::host::bossrush::RushRun>, cur: &crate::host::bossrush::RushRun) -> Vec<String> {
    let mut out = Vec::new();
    let Some(p) = prev else {
        out.push(format!("phase {}", cur.phase));
        return out;
    };
    if p.phase != cur.phase {
        out.push(format!("phase {} -> {}", p.phase, cur.phase));
    }
    for m in cur.marks.iter().skip(if p.phase == cur.phase || cur.phase == "fighting" { p.marks.len().min(cur.marks.len()) } else { 0 }) {
        out.push(format!("boss phase cast: {m}"));
    }
    if cur.done > p.done {
        out.push(format!("{} of {} down", cur.done, cur.of));
    }
    if cur.wipes > p.wipes {
        out.push(format!("wipe at {}", crate::host::bossrush::clock(cur.ms)));
    }
    if cur.clears > p.clears {
        out.push(format!("clear in {}", crate::host::bossrush::clock(cur.ms)));
    }
    out
}

/// The fight's verdict: it passes when it ran from the pull to an end (a kill or a wipe, said as it was) and Retry
/// had the raid ready in front of the boss again. Bots that can't win don't fail it; a fight that never started,
/// never ended or couldn't be retried does.
pub fn judge_fight(engaged: bool, outcome: Option<(&str, u64)>, phases: &[String], retry: Result<String, String>) -> (bool, String) {
    let ph = if phases.is_empty() { "none seen".to_string() } else { phases.join(", ") };
    let Some((what, ms)) = outcome else {
        return (false, format!("{}; phases: {ph}", if engaged { "the fight never ended in time" } else { "the boss never engaged" }));
    };
    let told = if what == "clear" { format!("KILL in {}", crate::host::bossrush::clock(ms)) } else { format!("WIPE at {}", crate::host::bossrush::clock(ms)) };
    match retry {
        Ok(r) => (engaged, format!("outcome: {told}; phases: {ph}; retry: {r}")),
        Err(e) => (false, format!("outcome: {told}; phases: {ph}; retry FAILED: {e}")),
    }
}

/// `levelup,companions,stoponexit,repair` (any order, `all` for every one) into the run order.
pub fn parse_checks(text: &str) -> Result<Vec<Check>, String> {
    let mut want = Vec::new();
    for w in text.split(',').map(|w| w.trim().to_ascii_lowercase()).filter(|w| !w.is_empty()) {
        match w.as_str() {
            "all" => want.extend(ALL),
            other => match ALL.iter().find(|c| c.word() == other) {
                Some(c) => want.push(*c),
                None => return Err(format!("--checks doesn't know {other:?} (levelup, companions, stoponexit, repair, all)")),
            },
        }
    }
    if want.is_empty() {
        return Err("--checks needs at least one check".into());
    }
    Ok(ALL.into_iter().filter(|c| want.contains(c)).collect())
}

// ------------------------------------------------------------------------------------------------ the SQL

fn login_ok(user: &str) -> bool {
    (3..=16).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The login's newest character, online first: `guid, name, level, online` (characters database).
pub fn in_world_sql(user: &str) -> Result<String, String> {
    if !login_ok(user) {
        return Err("That login doesn't look right.".into());
    }
    Ok(format!(
        "SELECT c.guid, c.name, c.level, c.online FROM characters c JOIN acore_auth.account a ON a.id = c.account \
         WHERE a.username = '{}' AND c.deleteInfos_Name IS NULL ORDER BY c.online DESC, c.guid DESC LIMIT 1",
        user.to_ascii_uppercase()
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Char {
    pub guid: u64,
    pub name: String,
    pub level: u8,
    pub online: bool,
}

/// [`in_world_sql`]'s row (`-N -B`: tab separated).
pub fn parse_char(rows: &str) -> Option<Char> {
    let line = rows.lines().map(|l| l.trim_end_matches('\r')).find(|l| !l.trim().is_empty())?;
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 4 || !crate::hybrid::char_name_ok(f[1]) {
        return None;
    }
    Some(Char { guid: f[0].trim().parse().ok()?, name: f[1].to_string(), level: f[2].trim().parse().ok()?, online: f[3].trim() != "0" })
}

/// One character's level by its guid.
pub fn level_sql(guid: u64) -> String {
    format!("SELECT level FROM characters WHERE guid = {guid}")
}

/// The bots the companions script keeps for this character (mod_companions_owned), with their level, whether they are
/// online and whether they share a group with the character (group_member): `name, kind, role, level, online, grouped`.
pub fn companions_sql(owner: u64) -> String {
    format!(
        "SELECT o.name, o.kind, o.role, c.level, c.online, \
         (SELECT COUNT(*) FROM group_member g1 JOIN group_member g2 ON g2.guid = g1.guid \
          WHERE g1.memberGuid = {owner} AND g2.memberGuid = o.bot) \
         FROM mod_companions_owned o JOIN characters c ON c.guid = o.bot WHERE o.owner = {owner} ORDER BY o.at, o.name"
    )
}

/// Everyone else online in the character's group (bots made any way: the script's table aside).
pub fn group_online_sql(owner: u64) -> String {
    format!(
        "SELECT c.name, c.level FROM group_member g1 JOIN group_member g2 ON g2.guid = g1.guid \
         JOIN characters c ON c.guid = g2.memberGuid WHERE g1.memberGuid = {owner} AND g2.memberGuid <> {owner} AND c.online <> 0 \
         ORDER BY c.name"
    )
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Companion {
    pub name: String,
    pub kind: String,
    pub role: String,
    pub level: u8,
    pub online: bool,
    pub grouped: bool,
}

pub fn parse_companions(rows: &str) -> Vec<Companion> {
    rows.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            if f.len() < 6 || !crate::hybrid::char_name_ok(f[0]) {
                return None;
            }
            Some(Companion {
                name: f[0].into(),
                kind: f[1].into(),
                role: f[2].into(),
                level: f[3].trim().parse().ok()?,
                online: f[4].trim() != "0",
                grouped: f[5].trim().parse::<u32>().ok()? > 0,
            })
        })
        .collect()
}

/// [`group_online_sql`]'s rows: `(name, level)`.
pub fn parse_group(rows: &str) -> Vec<(String, u8)> {
    rows.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
            if f.len() < 2 || !crate::hybrid::char_name_ok(f[0]) {
                return None;
            }
            Some((f[0].to_string(), f[1].trim().parse::<u8>().ok()?))
        })
        .collect()
}

/// The companions with the character now: in its group and online.
pub fn with_player(c: &[Companion]) -> Vec<&Companion> {
    c.iter().filter(|c| c.online && c.grouped && c.kind != "temp").collect()
}

/// How many companions join by themselves in this world, from its environment as the world server gets it
/// (engine::world_env: the profile, modules.env, the world's realm.env): 0 when bots or their own joining are off;
/// else `OV_COMPANIONS_FOLLOW` (2) held to 0..4 and to `OV_COMPANIONS_PER_PLAYER` (ov_companions.lua autoCount).
pub fn expected_companions(env: &[(String, String)]) -> u32 {
    let get = |k: &str| env.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v.trim().to_string());
    let on = |k: &str, d: bool| match get(k).as_deref() {
        Some("1") | Some("true") => true,
        Some("0") | Some("false") => false,
        _ => d,
    };
    let num = |k: &str, d: u32, hi: u32| get(k).and_then(|v| v.parse::<u32>().ok()).unwrap_or(d).min(hi);
    if !on("OV_COMPANIONS", true) || !on("OV_COMPANIONS_AUTO", false) {
        return 0;
    }
    num("OV_COMPANIONS_FOLLOW", 2, 4).min(num("OV_COMPANIONS_PER_PLAYER", 4, 4))
}

/// The console command that sets one character's level (the world's own GM login over SOAP, 127.0.0.1 only).
/// `.levelup` is in-game only; `character level` is the console's, and works on a character in the world.
pub fn levelup_command(name: &str, level: u8) -> Result<String, String> {
    if !crate::hybrid::char_name_ok(name) || !(2..=80).contains(&level) {
        return Err("That character's name or level doesn't look right.".into());
    }
    Ok(format!("character level {name} {level}"))
}

/// The worldserver log's line for a character entering the world (`Login Character:[Name]`).
pub fn log_in_world(text: &str, name: &str) -> bool {
    !name.is_empty() && text.contains(&format!("Login Character:[{name}]"))
}

/// Every companion with the player is at the player's level.
pub fn follow_level(with: &[(String, u8)], level: u8) -> bool {
    !with.is_empty() && with.iter().all(|(_, l)| *l == level)
}

// ------------------------------------------------------------------------------------------------ Repair's victim

/// The engine file Repair's check deletes: one the world can run a minute without and no program holds open (a
/// database update, a script, a text), never a program, a library, a `.conf` or the list itself. From engine.json's
/// list when there is one, else from the files found in the engine folder (`rel` paths with `/`).
pub fn pick_victim<'a>(files: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut best: Option<(u8, &str)> = None;
    for f in files {
        let low = f.to_ascii_lowercase();
        let name = low.rsplit('/').next().unwrap_or(&low);
        if name == "engine.json" || name.starts_with('.') {
            continue;
        }
        let rank = match name.rsplit('.').next().unwrap_or("") {
            "txt" | "md" => 0,
            "sql" => 1,
            "dist" => 2,
            "lua" => 3,
            _ => continue,
        };
        if best.map_or(true, |(r, b)| rank < r || (rank == r && f < b)) {
            best = Some((rank, f));
        }
    }
    best.map(|(_, f)| f.to_string())
}

/// Every file under `root` as a `/` path (Repair's victim when the engine has no list).
pub fn files_under(root: &Path) -> Vec<String> {
    fn walk(d: &Path, base: &Path, out: &mut Vec<String>, n: &mut usize) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            *n += 1;
            if *n > 50_000 {
                return;
            }
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out, n);
            } else if let Ok(r) = p.strip_prefix(base) {
                out.push(r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out, &mut 0);
    out.sort();
    out
}

pub fn rel_path(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

// ------------------------------------------------------------------------------------------------ the report

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    pub name: String,
    /// true pass, false fail, None information only (never fails the run).
    pub ok: Option<bool>,
    /// How long the step took.
    pub ms: u64,
    /// One line: what was seen, or why it failed.
    pub why: String,
}

/// The report: every step with pass / fail / time and a line, the phase the screenshots follow, and what was played.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub mark: &'static str,
    pub kind: String,
    pub checks: Vec<Check>,
    pub phase: String,
    pub pid: u32,
    pub started_ms: u64,
    pub done_ms: u64,
    pub steps: Vec<Step>,
    pub character: Option<Char>,
    pub companions: Vec<Companion>,
    pub levels: Vec<u8>,
    pub levelling: &'static str,
    pub account: String,
    pub hero: bool,
    pub passed: bool,
    pub extra: serde_json::Value,
}

impl Report {
    pub fn new(kind: &str, checks: &[Check], now_ms: u64) -> Report {
        Report {
            mark: SP_MARK,
            kind: kind.into(),
            checks: checks.to_vec(),
            phase: "start".into(),
            pid: 0,
            started_ms: now_ms,
            done_ms: 0,
            steps: Vec::new(),
            character: None,
            companions: Vec::new(),
            levels: Vec::new(),
            levelling: LEVEL_NOTE,
            account: String::new(),
            hero: false,
            passed: false,
            extra: serde_json::json!({}),
        }
    }

    pub fn step(&mut self, name: &str, ok: Option<bool>, ms: u64, why: impl Into<String>) {
        let why: String = why.into();
        self.steps.push(Step { name: name.into(), ok, ms, why: why.chars().take(400).collect() });
    }

    /// Every step passed (information lines aside) and there is at least one.
    pub fn all_passed(&self) -> bool {
        self.steps.iter().any(|s| s.ok == Some(true)) && !self.steps.iter().any(|s| s.ok == Some(false))
    }

    /// The steps a check needs, missing from the report (a run cut short): each one is a FAIL.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut need: Vec<&'static str> = vec!["setup", "in the world"];
        need.extend(self.checks.iter().map(|c| c.word()));
        need.into_iter().filter(|n| !self.steps.iter().any(|s| s.name == *n)).collect()
    }

    pub fn finish(&mut self, now_ms: u64) {
        for m in self.missing() {
            self.step(m, Some(false), 0, "not reached (an earlier step stopped the run)");
        }
        self.phase = "done".into();
        self.done_ms = now_ms;
        self.passed = self.all_passed();
    }

    /// One PASS / FAIL / INFO line per step, for the log.
    pub fn table(&self) -> String {
        self.steps
            .iter()
            .map(|s| {
                let k = match s.ok {
                    Some(true) => "PASS",
                    Some(false) => "FAIL",
                    None => "INFO",
                };
                format!("{k:<4} {:<22} {:>6.1}s  {}", s.name, s.ms as f64 / 1000.0, s.why)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_in_run_order() {
        assert_eq!(parse_checks("repair,levelup").unwrap(), vec![Check::Levelup, Check::Repair]);
        assert_eq!(parse_checks("all").unwrap(), ALL.to_vec());
        assert_eq!(parse_checks(" LevelUp , companions,levelup").unwrap(), vec![Check::Companions, Check::Levelup]);
        assert!(parse_checks("levelup,fly").is_err());
        assert!(parse_checks(",").is_err());
        assert_eq!(serde_json::to_string(&Check::StopOnExit).unwrap(), "\"stoponexit\"");
    }

    #[test]
    fn the_sql_takes_only_a_login_and_numbers() {
        let s = in_world_sql("hk2abcdefghijklm").unwrap();
        assert!(s.contains("a.username = 'HK2ABCDEFGHIJKLM'"), "{s}");
        assert!(s.contains("ORDER BY c.online DESC"));
        assert!(in_world_sql("SOLO' OR 1=1 --").is_err());
        assert!(in_world_sql("ab").is_err());
        assert!(level_sql(42).ends_with("guid = 42"));
        let c = companions_sql(7);
        assert!(c.contains("o.owner = 7") && c.contains("g1.memberGuid = 7") && c.contains("mod_companions_owned"));
        let g = group_online_sql(7);
        assert!(g.contains("g2.memberGuid <> 7") && g.contains("c.online <> 0"));
    }

    #[test]
    fn rows_read() {
        assert_eq!(parse_char("12\tAnna\t3\t1\r\n"), Some(Char { guid: 12, name: "Anna".into(), level: 3, online: true }));
        assert_eq!(parse_char("12\tAnna\t3\t0\n").unwrap().online, false);
        assert_eq!(parse_char(""), None);
        assert_eq!(parse_char("x\tAnna\t3\t1"), None);
        assert_eq!(parse_char("1\tDROP TABLE\t3\t1"), None);
        let c = parse_companions("Bo\tauto\ttank\t10\t1\t1\nCy\tauto\thealer\t9\t1\t0\nDi\ttemp\tdps\t10\t1\t1\nbad\n");
        assert_eq!(c.len(), 3);
        assert_eq!(with_player(&c).iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Bo"], "grouped, online, not a dungeon bot");
        assert_eq!(parse_group("Bo\t10\nCy\t9\n"), vec![("Bo".to_string(), 10), ("Cy".to_string(), 9)]);
        assert!(follow_level(&[("Bo".into(), 10), ("Cy".into(), 10)], 10));
        assert!(!follow_level(&[("Bo".into(), 10), ("Cy".into(), 9)], 10));
        assert!(!follow_level(&[], 10), "nobody to follow is not following");
    }

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn companions_expected_as_the_script_counts() {
        assert_eq!(expected_companions(&env(&[])), 0, "the script's default is off (the official realm)");
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1")])), 2);
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1"), ("OV_COMPANIONS_FOLLOW", "9")])), 4);
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1"), ("OV_COMPANIONS_FOLLOW", "3"), ("OV_COMPANIONS_PER_PLAYER", "1")])), 1);
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1"), ("OV_COMPANIONS", "0")])), 0);
        // the last line wins (realm.env is layered last)
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1"), ("OV_COMPANIONS_FOLLOW", "2"), ("OV_COMPANIONS_FOLLOW", "3")])), 3);
        assert_eq!(expected_companions(&env(&[("OV_COMPANIONS_AUTO", "1"), ("OV_COMPANIONS_AUTO", "0")])), 0);
    }

    #[test]
    fn the_solo_profiles_start_quiet() {
        // owner 12 Nov: a new world has no party until the player picks companions (host/spsettings.rs)
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../realm/profiles");
        for p in ["solo.env", "solovanilla.env"] {
            let Ok(text) = std::fs::read_to_string(root.join(p)) else { continue };
            assert_eq!(expected_companions(&crate::host::engine::parse_env(&text)), 0, "{p}");
            let chosen = format!("{text}\nOV_COMPANIONS_AUTO=1\nOV_COMPANIONS_FOLLOW=2\n");
            assert_eq!(expected_companions(&crate::host::engine::parse_env(&chosen)), 2, "{p} with companions chosen");
        }
    }

    #[test]
    fn levelup_only_for_a_name() {
        assert_eq!(levelup_command("Anna", 2).unwrap(), "character level Anna 2");
        assert!(levelup_command("Anna; .server shutdown", 2).is_err());
        assert!(levelup_command("Anna", 0).is_err());
        assert!(levelup_command("Anna", 81).is_err());
        assert!(log_in_world("Account: HK2 (IP: 127.0.0.1) Login Character:[Anna] (GUID: 5) Level: 1", "Anna"));
        assert!(!log_in_world("Login Character:[Annabel]", "Anna"));
        assert!(!log_in_world("Login Character:[]", ""));
    }

    #[test]
    fn the_victim_is_never_a_program() {
        let files = ["bin/worldserver.exe", "bin/libmysql.dll", "bin/worldserver.conf", "engine.json", "data/sql/updates/b.sql", "data/sql/updates/a.sql", "lua_scripts/x.lua"];
        assert_eq!(pick_victim(files).as_deref(), Some("data/sql/updates/a.sql"));
        assert_eq!(pick_victim(["bin/x.exe", "README.md", "a.sql"]).as_deref(), Some("README.md"));
        assert_eq!(pick_victim(["bin/x.exe", "bin/y.dll", "engine.json"]), None);
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("bin")).unwrap();
        std::fs::write(d.path().join("bin/w.exe"), b"MZ").unwrap();
        std::fs::write(d.path().join("bin/w.conf.dist"), b"x").unwrap();
        let f = files_under(d.path());
        assert_eq!(f, ["bin/w.conf.dist", "bin/w.exe"]);
        assert_eq!(pick_victim(f.iter().map(String::as_str)).as_deref(), Some("bin/w.conf.dist"));
        assert_eq!(rel_path(d.path(), "bin/w.exe"), d.path().join("bin").join("w.exe"));
    }

    #[test]
    fn boss_checks() {
        assert_eq!(parse_boss_checks("bossfight,bossstart").unwrap(), vec![Check::BossStart, Check::BossFight]);
        assert_eq!(parse_boss_checks("all").unwrap(), BOSS_ALL.to_vec());
        assert!(parse_boss_checks("levelup").is_err());
        assert!(parse_checks("bossfight").is_err(), "single player's list stays its own");
        assert_eq!(serde_json::to_string(&Check::BossFight).unwrap(), "\"bossfight\"");
        use crate::host::bossrush::RushRun;
        let mut r = RushRun { start: "frozen-throne".into(), diff: "normal".into(), phase: "ready".into(), leader: "Kalinra".into(), map: 631, instance: 3, dist: 3.0, members: 10, bots: 9, inside: 10, step: "frozen-throne".into(), of: 1, ..Default::default() };
        assert!(judge_start(&r, "frozen-throne", 631, 10).unwrap().contains("10 in the raid (9 companions, 10 inside)"));
        assert!(judge_start(&r, "icc-council", 631, 10).is_err());
        r.inside = 7;
        assert!(judge_start(&r, "frozen-throne", 631, 10).unwrap_err().contains("isn't complete"), "bots still arriving");
        r.inside = 10;
        r.dist = 40.0;
        assert!(judge_start(&r, "frozen-throne", 631, 10).unwrap_err().contains("not in front"));
        r.dist = 2.0;
        r.instance = 0;
        assert!(judge_start(&r, "frozen-throne", 631, 10).is_err());
        r.instance = 3;
        // the timeline
        let mut c = r.clone();
        assert_eq!(fight_events(None, &c), vec!["phase ready"]);
        c.phase = "fighting".into();
        c.marks = vec!["transition1".into()];
        assert_eq!(fight_events(Some(&r), &c), vec!["phase ready -> fighting", "boss phase cast: transition1"]);
        let mut d = c.clone();
        d.marks.push("quake".into());
        assert_eq!(fight_events(Some(&c), &d), vec!["boss phase cast: quake"]);
        let mut e = d.clone();
        e.phase = "wiped".into();
        e.wipes = 1;
        e.ms = 312_345;
        e.marks.clear();
        assert_eq!(fight_events(Some(&d), &e), vec!["phase fighting -> wiped", "wipe at 5:12.3"]);
        // the verdict: a wipe passes when it ran and Retry worked, and says so
        let ph = vec!["transition1".to_string()];
        let (ok, line) = judge_fight(true, Some(("wipe", 312_345)), &ph, Ok("in front of the boss again".into()));
        assert!(ok && line.contains("WIPE at 5:12.3") && line.contains("transition1"), "{line}");
        let (ok, line) = judge_fight(true, Some(("clear", 700_000)), &[], Ok("fresh raid".into()));
        assert!(ok && line.contains("KILL in 11:40.0") && line.contains("none seen"));
        assert!(!judge_fight(true, Some(("wipe", 1)), &ph, Err("still wiped".into())).0);
        assert!(!judge_fight(false, None, &[], Ok(String::new())).0);
        assert!(judge_fight(true, None, &[], Ok(String::new())).1.contains("never ended"));
    }

    #[test]
    fn the_report() {
        let mut r = Report::new("single", &[Check::Companions, Check::Levelup], 5);
        assert!(!r.all_passed(), "nothing ran: not a pass");
        r.step("setup", Some(true), 1200, "engine, maps, database, login");
        r.step("Hero Key", None, 0, "offline login (no test Hero Key)");
        r.step("in the world", Some(true), 3000, "Anna");
        assert_eq!(r.missing(), ["companions", "levelup"]);
        r.step("companions", Some(true), 9000, "2 of 2");
        r.step("levelup", Some(true), 9000, "1 to 10");
        r.finish(99);
        assert!(r.passed && r.phase == "done" && r.done_ms == 99);
        let mut cut = Report::new("vanilla", &[Check::Repair], 5);
        cut.step("setup", Some(true), 1, "");
        cut.finish(6);
        assert!(!cut.passed, "in the world and repair never ran");
        assert_eq!(cut.steps.iter().filter(|s| s.ok == Some(false)).count(), 2);
        let t = cut.table();
        assert!(t.contains("PASS setup") && t.contains("FAIL repair"), "{t}");
        let j = serde_json::to_value(&cut).unwrap();
        assert_eq!(j["mark"], SP_MARK);
        assert_eq!(j["levelling"], LEVEL_NOTE);
        assert!(LEVEL_NOTE.contains("not hand play"));
        let long = "x".repeat(1000);
        cut.step("x", None, 0, long);
        assert_eq!(cut.steps.last().unwrap().why.len(), 400);
    }
}
