//! The mods in a single player world (owner 12 Nov: "make sure it's easy to get mods from the mod store for
//! singleplayer and user has interface for that in the single player menu to see what mods they have on").
//!
//! A local world (Single player, the original world) runs the Built-in engine with every module built in
//! (`realm/modules.env`, layered with the world's profile, docs/BUILTIN-ENGINE.md). The mods list's realm mods never go
//! into a local world: their realm half is skipped on install (mods.rs) and runs only on a realm that installs it. So
//! the world-side mods are the engine's own modules, and the player turns the optional ones off or on here: the line
//! goes into the world's own `<world>/state/realm.env` (the same file as its settings, host/spsettings.rs), which
//! engine::world_env layers last at every start. A change applies at the world's next start (the page offers Restart).
//!
//! Nothing is downloaded and nothing of the engine is changed: a turned-off module stays in the engine, switched off.

use crate::host::engine::{self, Kind, Paths};
use crate::host::spsettings;
use serde::Serialize;
use std::fs;

/// One module the page lists: its id, the player's name for it, one line, its switch, and its default when the
/// engine isn't unpacked yet (what modules.env and the profiles set).
pub struct Def {
    pub id: &'static str,
    pub name: &'static str,
    pub line: &'static str,
    pub key: &'static str,
    /// Others depend on it: shown, never turned off here.
    pub locked: Option<&'static str>,
    pub default_on: bool,
}

// TODO(owner): his names and lines for the world's modules
pub const MODS: &[Def] = &[
    Def { id: "companions", name: "Companions and bots", line: "The bots that travel with you, fill your dungeons and roam the world.",
        key: "AC_PLAYERBOTS_ENABLED", locked: Some("Your companions and dungeon buddies need it. How many and when are in your world's settings."), default_on: true },
    Def { id: "scripts", name: "Leyline world scripts", line: "Leyline's own features in your world: the Friends tab, game modes and more.",
        key: "AC_ALE_ENABLED", locked: Some("Leyline's own world features run on it."), default_on: true },
    Def { id: "autobalance", name: "Group-size scaling", line: "Dungeons and raids scale to the size of your group.",
        key: "AC_AUTO_BALANCE_ENABLE_GLOBAL", locked: None, default_on: true },
    Def { id: "transmog", name: "Transmogrification", line: "Change how your gear looks and keep its stats.",
        key: "AC_TRANSMOGRIFICATION_ENABLE", locked: None, default_on: true },
    Def { id: "aoeloot", name: "Area loot", line: "Loot every corpse around you at once.",
        key: "AC_AOELOOT_ENABLE", locked: None, default_on: true },
    Def { id: "sololfg", name: "Solo dungeon finder", line: "Queue for a dungeon on your own.",
        key: "AC_SOLO_LFG_ENABLE", locked: None, default_on: true },
    Def { id: "challenge", name: "Challenge modes", line: "Hardcore, Self-Crafted, Slow XP and more, chosen per character.",
        key: "AC_CHALLENGE_MODES_ENABLE", locked: None, default_on: true },
    Def { id: "dungeonmaster", name: "Dungeon runs", line: "Random dungeon runs from a dungeon master in town.",
        key: "AC_DUNGEON_MASTER_ENABLE", locked: None, default_on: true },
    Def { id: "arena1v1", name: "One-on-one arena", line: "Arena matches of one against one.",
        key: "AC_ARENA_1_V_1__ENABLE", locked: None, default_on: true },
    Def { id: "keystones", name: "Keystone dungeons", line: "Timed dungeons with keystones and affixes.",
        key: "AC_MYTHIC_PLUS_ENABLE", locked: None, default_on: true },
    Def { id: "questdata", name: "Quest helper data", line: "Gives the quest helper add-on live data from your world.",
        key: "AC_QUESTIE_BRIDGE_ENABLED", locked: None, default_on: true },
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorldMod {
    pub id: String,
    pub name: String,
    pub line: String,
    pub on: bool,
    /// Shown, but not turned off here (`note` says why).
    pub locked: bool,
    pub note: String,
    /// The player turned it off or on here (the world's realm.env names it).
    pub mine: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorldMods {
    pub kind: String,
    pub mods: Vec<WorldMod>,
    pub set_up: bool,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Saved {
    pub mods: Vec<WorldMod>,
    /// The world is running: the change applies at its next start (the page offers Restart now).
    pub restart: bool,
    pub message: String,
}

/// The worlds that list them (Boss Rush hands out its own rules).
pub fn supports(kind: Kind) -> bool {
    spsettings::supports(kind)
}

fn read(path: &std::path::Path) -> Vec<(String, String)> {
    engine::parse_env(&fs::read_to_string(path).unwrap_or_default())
}

fn val<'a>(lines: &'a [(String, String)], k: &str) -> Option<&'a str> {
    lines.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
}

fn flag(v: Option<&str>) -> Option<bool> {
    match v.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "1" || s == "true" => Some(true),
        Some(s) if s == "0" || s == "false" => Some(false),
        _ => None,
    }
}

/// The engine's own lines (before the player's realm.env), layered as engine::world_env does.
fn base(p: &Paths) -> Vec<(String, String)> {
    let profile = read(&p.profile_env(p.kind.profile()));
    let modules = read(&p.modules_env());
    if p.kind.profile_last() {
        modules.into_iter().chain(profile).collect()
    } else {
        profile.into_iter().chain(modules).collect()
    }
}

/// The world's modules as its server gets them at its next start.
pub fn list(p: &Paths) -> Vec<WorldMod> {
    let base = base(p);
    let realm = read(&p.realm_env());
    MODS.iter()
        .map(|d| {
            let mine = flag(val(&realm, d.key));
            let on = mine.or_else(|| flag(val(&base, d.key))).unwrap_or(d.default_on);
            WorldMod {
                id: d.id.into(),
                name: d.name.into(),
                line: d.line.into(),
                on: if d.locked.is_some() { true } else { on },
                locked: d.locked.is_some(),
                note: d.locked.unwrap_or("").into(),
                mine: mine.is_some() && d.locked.is_none(),
            }
        })
        .collect()
}

/// Turn one module off or on in the world's realm.env (every other line kept). `running`: the world runs now.
pub fn set(p: &Paths, id: &str, on: bool, running: bool) -> Result<Saved, String> {
    if !supports(p.kind) {
        return Err(format!("{} has no mods to turn off.", p.kind.title()));
    }
    let d = MODS.iter().find(|d| d.id == id).ok_or("That isn't one of your world's mods.")?;
    if let Some(why) = d.locked {
        return Err(format!("{} stays on. {why}", d.name));
    }
    // the same spelling as the engine's own line (mod-ale reads true/false, the others 1/0)
    let had = val(&base(p), d.key).map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let v = if had == "true" || had == "false" { if on { "true" } else { "false" } } else if on { "1" } else { "0" };
    fs::create_dir_all(&p.state).map_err(|e| format!("Couldn't make the world's folder: {e}"))?;
    let text = fs::read_to_string(p.realm_env()).unwrap_or_default();
    let t = spsettings::merge(&text, &[(d.key.to_string(), v.to_string())]);
    crate::settings::write_atomic(&p.realm_env(), t.as_bytes()).map_err(|e| format!("Couldn't save your world's mods: {e}"))?;
    let what = if on { "on" } else { "off" };
    let message = if running {
        format!("{} is {what}. {} is running: it changes at its next start (Restart now, or the next time you play).", d.name, p.kind.title())
    } else {
        format!("{} is {what}. It changes when your world starts.", d.name)
    };
    Ok(Saved { mods: list(p), restart: running, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(d: &std::path::Path, k: Kind) -> Paths {
        Paths::for_kind(d, k)
    }

    fn unpack(p: &Paths, modules: &str, profile: &str) {
        fs::create_dir_all(p.dist.join("realm").join("profiles")).unwrap();
        fs::write(p.modules_env(), modules).unwrap();
        fs::write(p.profile_env(p.kind.profile()), profile).unwrap();
    }

    fn on(v: &[WorldMod], id: &str) -> bool {
        v.iter().find(|m| m.id == id).unwrap().on
    }

    #[test]
    fn a_world_not_unpacked_yet_lists_the_defaults() {
        let d = tempfile::tempdir().unwrap();
        let v = list(&paths(d.path(), Kind::Single));
        assert_eq!(v.len(), MODS.len());
        assert!(v.iter().all(|m| m.on && !m.mine));
        assert!(v.iter().find(|m| m.id == "companions").unwrap().locked);
        assert!(!v.iter().find(|m| m.id == "transmog").unwrap().locked);
    }

    #[test]
    fn the_engine_lines_layer_as_the_world_server_reads_them() {
        let d = tempfile::tempdir().unwrap();
        // Single player: the profile first, modules.env after (its line wins)
        let p = paths(d.path(), Kind::Single);
        unpack(&p, "AC_AOELOOT_ENABLE=1\nAC_SOLO_LFG_ENABLE=0\n", "AC_AOELOOT_ENABLE=0\n");
        let v = list(&p);
        assert!(on(&v, "aoeloot"), "modules.env wins for Single player");
        assert!(!on(&v, "sololfg"));
        // the original world: the profile last (its line wins)
        let q = paths(d.path(), Kind::Vanilla);
        fs::write(q.profile_env(q.kind.profile()), "AC_AOELOOT_ENABLE=0\n").unwrap();
        assert!(!on(&list(&q), "aoeloot"), "the profile wins for the original world");
    }

    #[test]
    fn turning_one_off_writes_only_its_line_and_keeps_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        unpack(&p, "AC_TRANSMOGRIFICATION_ENABLE=1\nAC_ALE_ENABLED=true\n", "");
        fs::create_dir_all(&p.state).unwrap();
        fs::write(p.realm_env(), "# mine\nWU_BOTS=7\nOV_COMPANIONS_FOLLOW=2\n").unwrap();
        let s = set(&p, "transmog", false, false).unwrap();
        assert!(!s.restart);
        assert!(!on(&s.mods, "transmog"));
        assert!(s.mods.iter().find(|m| m.id == "transmog").unwrap().mine);
        let t = fs::read_to_string(p.realm_env()).unwrap();
        for l in ["# mine", "WU_BOTS=7", "OV_COMPANIONS_FOLLOW=2", "AC_TRANSMOGRIFICATION_ENABLE=0"] {
            assert!(t.lines().any(|x| x == l), "{l} in {t}");
        }
        // the world server gets it
        let modules = fs::read_to_string(p.modules_env()).unwrap();
        let env = engine::world_env(&p, &engine::State::default(), "", &modules, Some(&t));
        assert!(env.iter().any(|(k, v)| k == "AC_TRANSMOGRIFICATION_ENABLE" && v == "0"));
        // and back on: one line, not two
        set(&p, "transmog", true, false).unwrap();
        let t = fs::read_to_string(p.realm_env()).unwrap();
        assert_eq!(t.lines().filter(|l| l.starts_with("AC_TRANSMOGRIFICATION_ENABLE=")).count(), 1);
        assert!(t.lines().any(|l| l == "AC_TRANSMOGRIFICATION_ENABLE=1"));
    }

    #[test]
    fn a_true_false_switch_keeps_its_spelling() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        unpack(&p, "AC_AUTO_BALANCE_ENABLE_GLOBAL=true\n", "");
        set(&p, "autobalance", false, false).unwrap();
        let t = fs::read_to_string(p.realm_env()).unwrap();
        assert!(t.lines().any(|l| l == "AC_AUTO_BALANCE_ENABLE_GLOBAL=false"), "{t}");
    }

    #[test]
    fn a_running_world_says_next_start() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        let s = set(&p, "aoeloot", false, true).unwrap();
        assert!(s.restart);
        assert!(s.message.contains("next start"), "{}", s.message);
    }

    #[test]
    fn locked_unknown_and_boss_rush_are_refused_and_write_nothing() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        assert!(set(&p, "companions", false, false).is_err());
        assert!(set(&p, "scripts", false, false).is_err());
        assert!(set(&p, "nope", false, false).is_err());
        assert!(!p.realm_env().exists(), "nothing written");
        let b = paths(d.path(), Kind::BossRush);
        assert!(!supports(Kind::BossRush));
        assert!(set(&b, "transmog", false, false).is_err());
        assert!(!b.realm_env().exists());
    }

    #[test]
    fn the_names_carry_no_brand_words_and_every_key_is_in_modules_env() {
        let brands = ["world of warcraft", "warcraft", "wow ", "lich king", "blizzard"];
        for d in MODS {
            let t = format!("{} {} {}", d.name, d.line, d.locked.unwrap_or("")).to_ascii_lowercase();
            assert!(!brands.iter().any(|b| t.contains(b)), "{t}");
        }
        // the switches are the realm's own (the engine's modules.env is made from it)
        if let Ok(m) = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../realm/modules.env")) {
            let lines = engine::parse_env(&m);
            for d in MODS {
                assert!(val(&lines, d.key).is_some(), "{} in realm/modules.env", d.key);
            }
        }
    }
}
