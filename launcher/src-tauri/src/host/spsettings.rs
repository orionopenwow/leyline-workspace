//! Single player's world settings (owner 12 Nov: "let the user customize those settings in the single player option
//! menu before loading their world"): companions that travel with the player, dungeon buddies, the world's roaming
//! bots and the XP rate, kept in the world's own `<world>/state/realm.env` (engine::Paths::realm_env), which
//! engine::world_env layers last at every start. A change applies at the world's next start.
//!
//! A new world no longer starts with a party and a busy world by itself: until the player chooses, companions don't
//! join by themselves, dungeon buddies fill dungeons only, and no roaming bots ([`NEW`]). A world set up before these
//! settings existed keeps what it had ([`LEGACY`]): [`settle`] writes it into its realm.env once, before its server
//! next reads it. Keys: docs/SINGLE-PLAYER-ONE-CLICK.md "Companions settings".

use crate::host::engine::{self, Kind, Paths, State};
use serde::Serialize;
use std::fs;

pub const AUTO: &str = "OV_COMPANIONS_AUTO";
pub const FOLLOW: &str = "OV_COMPANIONS_FOLLOW";
pub const DUNGEON: &str = "OV_COMPANIONS_DUNGEON_AUTO";
pub const BOTS: &str = "WU_BOTS";
/// Every XP rate the setting sets together (the profiles set all four alike).
pub const XP_KEYS: [&str; 4] = ["AC_RATE_XP_KILL", "AC_RATE_XP_QUEST", "AC_RATE_XP_QUEST_DF", "AC_RATE_XP_EXPLORE"];
pub const MAX_FOLLOW: u8 = 4;
pub const MAX_BOTS: u32 = 500;
pub const MIN_XP: f64 = 0.5;
pub const MAX_XP: f64 = 10.0;
/// The profiles' own XP rate (solo.env, solovanilla.env) when the engine isn't unpacked yet.
pub const PROFILE_XP: f64 = 2.0;

/// The companion settings with no XP (that one stays the profile's unless chosen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub auto: bool,
    pub follow: u8,
    pub dungeon: bool,
    pub bots: u32,
}

/// A new world: quiet, nobody joins by themselves, dungeon buddies only inside dungeons.
pub const NEW: Defaults = Defaults { auto: false, follow: 0, dungeon: true, bots: 0 };

/// What a world set up before the settings had (the profiles' old lines; Single player's 20 roaming bots).
pub fn legacy(kind: Kind) -> Defaults {
    Defaults { auto: true, follow: 2, dungeon: true, bots: if kind == Kind::Single { engine::BOTS } else { 0 } }
}

/// The worlds with companions (Boss Rush hands out its own heroes and has none of these).
pub fn supports(kind: Kind) -> bool {
    matches!(kind, Kind::Single | Kind::Vanilla)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorldSettings {
    pub kind: String,
    pub companions_auto: bool,
    pub companions_follow: u8,
    pub dungeon_auto: bool,
    pub roaming_bots: u32,
    pub xp_rate: f64,
    /// The player chose them (the "Your world" panel isn't shown again).
    pub chosen: bool,
    pub set_up: bool,
    pub running: bool,
}

/// What `world_settings_set` changes (None: kept).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Change {
    pub companions_auto: Option<bool>,
    pub companions_follow: Option<u8>,
    pub dungeon_auto: Option<bool>,
    pub roaming_bots: Option<u32>,
    pub xp_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Saved {
    pub settings: WorldSettings,
    /// The world is running: the change applies at its next start (the page offers Restart).
    pub restart: bool,
    pub message: String,
}

fn val<'a>(lines: &'a [(String, String)], k: &str) -> Option<&'a str> {
    lines.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
}

fn flag(v: Option<&str>) -> Option<bool> {
    match v.map(str::trim) {
        Some("1") | Some("true") => Some(true),
        Some("0") | Some("false") => Some(false),
        _ => None,
    }
}

fn read_realm(p: &Paths) -> String {
    fs::read_to_string(p.realm_env()).unwrap_or_default()
}

/// "1.5", "2": the shortest form.
pub fn fmt_rate(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// The settings as the world's server gets them: realm.env over the defaults (a world not settled yet: the quiet
/// ones when new, its old ones when it was set up before), the XP rate over the profile's.
pub fn current(p: &Paths, s: &State) -> WorldSettings {
    let realm = engine::parse_env(&read_realm(p));
    let d = if !s.settings_settled && s.filled { legacy(p.kind) } else { NEW };
    let profile = engine::parse_env(&fs::read_to_string(p.profile_env(p.kind.profile())).unwrap_or_default());
    let xp = val(&realm, XP_KEYS[0]).or_else(|| val(&profile, XP_KEYS[0])).and_then(|x| x.trim().parse::<f64>().ok()).unwrap_or(PROFILE_XP);
    let auto = flag(val(&realm, AUTO)).unwrap_or(d.auto);
    let follow = val(&realm, FOLLOW).and_then(|x| x.trim().parse::<u8>().ok()).unwrap_or(d.follow).min(MAX_FOLLOW);
    WorldSettings {
        kind: p.kind.word().into(),
        companions_auto: auto,
        companions_follow: follow,
        dungeon_auto: flag(val(&realm, DUNGEON)).unwrap_or(d.dungeon),
        roaming_bots: val(&realm, BOTS).and_then(|x| x.trim().parse::<u32>().ok()).unwrap_or(d.bots).min(MAX_BOTS),
        xp_rate: xp,
        chosen: s.settings_chosen,
        set_up: false,
        running: false,
    }
}

/// `text` with `pairs` set: a key's existing line replaced where it is (the last one if it is there twice, earlier
/// copies dropped), new keys added at the end; every other line (comments, the player's own) kept as it was.
pub fn merge(text: &str, pairs: &[(String, String)]) -> String {
    let key_of = |l: &str| -> Option<String> {
        let t = l.trim_end_matches('\r').trim_start();
        if t.starts_with('#') {
            return None;
        }
        let t = t.strip_prefix("export ").unwrap_or(t);
        t.split_once('=').map(|(k, _)| k.trim().to_string())
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        match key_of(l).and_then(|k| pairs.iter().find(|(x, _)| *x == k).map(|p| (k, p))) {
            Some((k, (_, v))) => {
                // only the last line of a key stays (as the layering reads it)
                if lines[i + 1..].iter().any(|n| key_of(n).as_deref() == Some(k.as_str())) {
                    continue;
                }
                out.push(format!("{k}={v}"));
            }
            None => out.push(l.trim_end_matches('\r').to_string()),
        }
    }
    for (k, v) in pairs {
        if !lines.iter().any(|l| key_of(l).as_deref() == Some(k.as_str())) {
            out.push(format!("{k}={v}"));
        }
    }
    let mut t = out.join("\n");
    t.push('\n');
    t
}

fn write_realm(p: &Paths, pairs: &[(String, String)]) -> Result<(), String> {
    fs::create_dir_all(&p.state).map_err(|e| format!("Couldn't make the world's folder: {e}"))?;
    let t = merge(&read_realm(p), pairs);
    crate::settings::write_atomic(&p.realm_env(), t.as_bytes()).map_err(|e| format!("Couldn't save the world's settings: {e}"))
}

fn lines_of(d: Defaults) -> Vec<(String, String)> {
    vec![
        (AUTO.into(), if d.auto { "1" } else { "0" }.into()),
        (FOLLOW.into(), d.follow.to_string()),
        (DUNGEON.into(), if d.dungeon { "1" } else { "0" }.into()),
        (BOTS.into(), d.bots.to_string()),
    ]
}

/// Once per world, before its server reads realm.env: the keys it doesn't name yet written there (a new world's quiet
/// defaults; a world set up before the settings existed keeps what it had). True when `s` changed (save it).
pub fn settle(p: &Paths, s: &mut State) -> bool {
    if !supports(p.kind) || s.settings_settled {
        return false;
    }
    let d = if s.filled { legacy(p.kind) } else { NEW };
    let have = engine::parse_env(&read_realm(p));
    let missing: Vec<(String, String)> = lines_of(d).into_iter().filter(|(k, _)| val(&have, k).is_none()).collect();
    if !missing.is_empty() && write_realm(p, &missing).is_err() {
        return false;
    }
    s.settings_settled = true;
    true
}

/// Check a change before anything is written.
pub fn validate(c: &Change) -> Result<(), String> {
    if let Some(f) = c.companions_follow {
        if f > MAX_FOLLOW {
            return Err(format!("Companions: 0 to {MAX_FOLLOW}."));
        }
    }
    if let Some(b) = c.roaming_bots {
        if b > MAX_BOTS {
            return Err(format!("Roaming bots: 0 to {MAX_BOTS}."));
        }
    }
    if let Some(x) = c.xp_rate {
        if !x.is_finite() || !(MIN_XP..=MAX_XP).contains(&x) {
            return Err(format!("XP rate: {} to {}.", fmt_rate(MIN_XP), fmt_rate(MAX_XP)));
        }
    }
    Ok(())
}

/// Write the settings named in `c` into the world's realm.env (the others kept), and remember they were chosen.
/// `running`: the world runs now (the change applies at its next start).
pub fn set(p: &Paths, c: &Change, running: bool) -> Result<Saved, String> {
    if !supports(p.kind) {
        return Err(format!("{} has no world settings.", p.kind.title()));
    }
    validate(c)?;
    let mut s = engine::load_state(p);
    // a world never settled gets its defaults first, so what isn't chosen now stays what it was
    settle(p, &mut s);
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(a) = c.companions_auto {
        pairs.push((AUTO.into(), if a { "1" } else { "0" }.into()));
    }
    if let Some(f) = c.companions_follow {
        pairs.push((FOLLOW.into(), f.to_string()));
    }
    if let Some(d) = c.dungeon_auto {
        pairs.push((DUNGEON.into(), if d { "1" } else { "0" }.into()));
    }
    if let Some(b) = c.roaming_bots {
        pairs.push((BOTS.into(), b.to_string()));
    }
    if let Some(x) = c.xp_rate {
        for k in XP_KEYS {
            pairs.push((k.into(), fmt_rate(x)));
        }
    }
    write_realm(p, &pairs)?;
    s.settings_chosen = true;
    s.settings_settled = true;
    engine::save_state(p, &s)?;
    let mut settings = current(p, &s);
    settings.running = running;
    let message = if running {
        format!("Saved. {} is running: they apply at its next start (Restart now, or the next time you play).", p.kind.title())
    } else {
        "Saved. They apply when your world starts.".to_string()
    };
    Ok(Saved { settings, restart: running, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(d: &std::path::Path, k: Kind) -> Paths {
        Paths::for_kind(d, k)
    }

    #[test]
    fn a_new_world_is_quiet_until_the_player_chooses() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        let s = State::default();
        let c = current(&p, &s);
        assert_eq!((c.companions_auto, c.companions_follow, c.dungeon_auto, c.roaming_bots), (false, 0, true, 0));
        assert_eq!(c.xp_rate, PROFILE_XP);
        assert!(!c.chosen);
        // the engine's own fallback agrees: no roaming bots unless named
        assert_eq!(Kind::Single.bots(), 0);
    }

    #[test]
    fn settle_writes_the_quiet_defaults_once_for_a_new_world() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        let mut s = State::default();
        assert!(settle(&p, &mut s));
        assert!(s.settings_settled && !s.settings_chosen);
        let t = fs::read_to_string(p.realm_env()).unwrap();
        for l in ["OV_COMPANIONS_AUTO=0", "OV_COMPANIONS_FOLLOW=0", "OV_COMPANIONS_DUNGEON_AUTO=1", "WU_BOTS=0"] {
            assert!(t.lines().any(|x| x == l), "{l} in {t}");
        }
        assert!(!settle(&p, &mut s), "once");
        // the engine's environment gets them
        let v = engine::world_env(&p, &s, "OV_COMPANIONS_AUTO=1\nOV_COMPANIONS_FOLLOW=2\n", "", Some(&t));
        assert!(v.iter().any(|(k, x)| k == "OV_COMPANIONS_AUTO" && x == "0"));
        assert!(v.iter().any(|(k, x)| k == "AC_PLAYERBOTS_MAX_RANDOM_BOTS" && x == "0"));
    }

    #[test]
    fn a_world_set_up_before_keeps_what_it_had() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        let mut s = State { filled: true, ..Default::default() };
        // its own realm.env line wins over the old defaults
        fs::create_dir_all(&p.state).unwrap();
        fs::write(p.realm_env(), "# mine\nWU_BOTS=7\nOV_PATHING=1\n").unwrap();
        let c = current(&p, &s);
        assert_eq!((c.companions_auto, c.companions_follow, c.roaming_bots), (true, 2, 7));
        assert!(settle(&p, &mut s));
        let t = fs::read_to_string(p.realm_env()).unwrap();
        assert!(t.starts_with("# mine\nWU_BOTS=7\nOV_PATHING=1\n"), "{t}");
        assert!(t.contains("OV_COMPANIONS_AUTO=1\n") && t.contains("OV_COMPANIONS_FOLLOW=2\n") && !t.contains("WU_BOTS=20"));
        // the original world had no roaming bots
        assert_eq!(legacy(Kind::Vanilla).bots, 0);
        assert_eq!(legacy(Kind::Single).bots, engine::BOTS);
    }

    #[test]
    fn set_writes_only_what_changed_and_keeps_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Vanilla);
        fs::create_dir_all(&p.state).unwrap();
        fs::write(p.realm_env(), "OV_PATHING=1\r\nOV_COMPANIONS_FOLLOW=3\nOV_COMPANIONS_FOLLOW=4\n").unwrap();
        let r = set(&p, &Change { companions_auto: Some(true), companions_follow: Some(1), xp_rate: Some(1.5), ..Default::default() }, false).unwrap();
        assert!(!r.restart);
        let t = fs::read_to_string(p.realm_env()).unwrap();
        assert!(t.starts_with("OV_PATHING=1\nOV_COMPANIONS_FOLLOW=1\n"), "{t}");
        assert_eq!(t.matches("OV_COMPANIONS_FOLLOW=").count(), 1);
        for k in XP_KEYS {
            assert!(t.contains(&format!("{k}=1.5\n")), "{k}");
        }
        assert!(t.contains("OV_COMPANIONS_AUTO=1\n") && t.contains("WU_BOTS=0\n"));
        let s = engine::load_state(&p);
        assert!(s.settings_chosen && s.settings_settled);
        assert_eq!((r.settings.companions_follow, r.settings.xp_rate, r.settings.chosen), (1, 1.5, true));
        // settle never overwrites a choice
        let mut s2 = s.clone();
        assert!(!settle(&p, &mut s2));
    }

    #[test]
    fn set_refuses_out_of_range_values_and_boss_rush() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        assert!(set(&p, &Change { companions_follow: Some(5), ..Default::default() }, false).is_err());
        assert!(set(&p, &Change { roaming_bots: Some(501), ..Default::default() }, false).is_err());
        assert!(set(&p, &Change { xp_rate: Some(0.1), ..Default::default() }, false).is_err());
        assert!(set(&p, &Change { xp_rate: Some(f64::NAN), ..Default::default() }, false).is_err());
        assert!(!p.realm_env().exists(), "nothing written");
        assert!(set(&paths(d.path(), Kind::BossRush), &Change::default(), false).is_err());
    }

    #[test]
    fn a_running_world_says_next_start() {
        let d = tempfile::tempdir().unwrap();
        let p = paths(d.path(), Kind::Single);
        let r = set(&p, &Change { roaming_bots: Some(60), ..Default::default() }, true).unwrap();
        assert!(r.restart && r.message.contains("next start"));
        assert_eq!(r.settings.roaming_bots, 60);
        let low = r.message.to_lowercase();
        assert!(!low.contains("warcraft") && !low.contains("blizzard"));
    }

    #[test]
    fn rates_print_short() {
        assert_eq!(fmt_rate(2.0), "2");
        assert_eq!(fmt_rate(1.5), "1.5");
        assert_eq!(merge("", &[("A".into(), "1".into())]), "A=1\n");
    }
}
