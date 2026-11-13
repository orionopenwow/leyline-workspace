//! Single player on a game folder without Leyline's login screen code (a stock game, no Forever UI): the launcher
//! types what a player would, so PLAY still lands in the world with nothing to type.
//!
//! The same [`Driver`] as Your Den's one click (crate::denclick) decides the next key; only where the milestones come
//! from differs. Den Lite writes its own log; a single player world's are read from its database once a second:
//!
//! - `auth_ok`: the login's `last_login` moved past this start (the login server took the password);
//! - `world_auth` + `char_enum`: `account.online` is set (the world took the session: character select is up);
//! - `in_world`: one of the login's characters is `online`.
//!
//! First entry (no character yet): the game opens its create screen with a race and class already picked and the
//! name box focused, so the launcher types the name and Enter (made in the game, exactly as a player does), then
//! Enter World. Windows only; everything waits on a milestone and gives up after a timeout, leaving the game as it
//! is. What it did is in `<world>/logs/local-click.log`. The password is never logged.
//!
//! The sign-in cover (crate::cover, Mode::Single) follows the same milestones: [`drive`] tells it how far the
//! sign-in is and how it ended; where Leyline's login screen code signs in instead (crate::hybrid), [`follow`] only
//! reads the milestones for it, typing nothing.

use crate::denclick::{self as dc, Act, Driver, Ev, Game, Key};
use crate::host::engine as eng;
use std::path::Path;

/// Where it writes what it did (one start).
pub const LOG: &str = "local-click.log";

/// One read of the world's database for the login.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Row {
    /// `account.last_login`, unix seconds (0 when never).
    pub last_login: u64,
    pub online: bool,
    pub chars: u32,
    pub chars_online: bool,
}

/// The one query (the login is A–Z0–9 only, checked before).
pub fn row_sql(account: &str) -> Result<String, String> {
    if account.is_empty() || account.len() > 16 || !account.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        return Err("That login name can't be asked about.".into());
    }
    Ok(format!(
        "SELECT COALESCE(UNIX_TIMESTAMP(a.last_login),0), a.online, \
         (SELECT COUNT(*) FROM acore_characters.characters c WHERE c.account = a.id), \
         (SELECT COALESCE(MAX(c.online),0) FROM acore_characters.characters c WHERE c.account = a.id) \
         FROM acore_auth.account a WHERE a.username = '{account}'"
    ))
}

pub fn parse_row(out: &str) -> Option<Row> {
    let line = out.lines().find(|l| !l.trim().is_empty())?;
    let f: Vec<&str> = line.split('\t').map(str::trim).collect();
    if f.len() < 4 {
        return None;
    }
    let n = |s: &str| s.split('.').next().unwrap_or("0").parse::<u64>().ok();
    Some(Row { last_login: n(f[0])?, online: n(f[1])? > 0, chars: n(f[2])? as u32, chars_online: n(f[3])? > 0 })
}

/// The milestones so far, with the time each was first seen (the [`Driver`] waits from those times).
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub auth_ok: Option<u64>,
    pub char_enum: Option<u64>,
    pub in_world: Option<u64>,
}

impl Seen {
    /// Fold one read in. `start` is this start (unix ms); a login before it is an earlier game's.
    pub fn take(&mut self, r: &Row, start: u64, now: u64) {
        if self.auth_ok.is_none() && r.last_login > 0 && r.last_login * 1000 + 2_000 >= start {
            self.auth_ok = Some(now);
        }
        if self.char_enum.is_none() && r.online && self.auth_ok.is_some() {
            self.char_enum = Some(now);
        }
        if self.in_world.is_none() && r.chars_online && self.char_enum.is_some() {
            self.in_world = Some(now);
        }
    }

    pub fn events(&self) -> Vec<Ev> {
        let ev = |t: u64, name: &str| Ev { t, ev: name.into(), who: String::new(), detail: String::new() };
        let mut v = Vec::new();
        if let Some(t) = self.auth_ok {
            v.push(ev(t, "auth_ok"));
        }
        if let Some(t) = self.char_enum {
            v.push(ev(t, "world_auth"));
            v.push(ev(t, "char_enum"));
        }
        if let Some(t) = self.in_world {
            v.push(ev(t, "in_world"));
        }
        v
    }
}

/// How far the sign-in is, for the cover (crate::cover::DenStage, the Den's milestones' order).
pub fn stage(seen: &Seen) -> crate::cover::DenStage {
    use crate::cover::DenStage;
    if seen.in_world.is_some() {
        DenStage::Entering
    } else if seen.char_enum.is_some() {
        DenStage::CharSelect
    } else if seen.auth_ok.is_some() {
        DenStage::Realm
    } else {
        DenStage::None
    }
}

/// How a [`drive`] went for the cover: through (in the world, or at character select with no character and no name
/// to make one: the player chose the game's own create screen), or not.
pub fn went_through(outcome: &str, seen: &Seen, chars: u32, name: Option<&str>) -> bool {
    outcome == "in the world" || (seen.char_enum.is_some() && chars == 0 && name.is_none())
}

/// [`follow`]: the longest it reads (ms); the cover has its own, shorter limit.
pub const FOLLOW_MAX: u64 = 120_000;
/// [`follow`] to character select: the list draws a moment after the world takes the session (ms).
pub const SELECT_GRACE: u64 = 900;

/// [`follow`]'s decision at one read: Some(true) through, Some(false) given up, None: read again.
/// `to_world`: our login screen code enters a character (else it stops at character select).
pub fn follow_done(seen: &Seen, to_world: bool, start: u64, now: u64) -> Option<bool> {
    if seen.in_world.is_some() {
        return Some(true);
    }
    if !to_world {
        if let Some(t) = seen.char_enum {
            if now.saturating_sub(t) >= SELECT_GRACE {
                return Some(true);
            }
        }
    }
    (now.saturating_sub(start) > FOLLOW_MAX).then_some(false)
}

/// Where Leyline's login screen code signs in (crate::hybrid): nothing typed, the world's database read once a second
/// for the sign-in cover only, until the character is in the world (or character select, `to_world` false), the game
/// closes, or [`FOLLOW_MAX`].
pub fn follow(p: &eng::Paths, account: &str, pid: u32, to_world: bool, cover: &crate::cover::Signals) {
    let start = dc::now_ms();
    let Ok(sql) = row_sql(account) else {
        return cover.finish(false);
    };
    let mut game = dc::Process(pid);
    let mut seen = Seen::default();
    loop {
        let now = dc::now_ms();
        if !game.alive() {
            return cover.finish(false);
        }
        if let Some(r) = eng::query(p, "acore_auth", &sql).ok().and_then(|o| parse_row(&o)) {
            seen.take(&r, start, now);
        }
        cover.set_stage(stage(&seen));
        if let Some(ok) = follow_done(&seen, to_world, start, now) {
            return cover.finish(ok);
        }
        std::thread::sleep(std::time::Duration::from_millis(1_000));
    }
}

/// What to do at character select with no character yet: type the name (once), or wait for the game to make it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Create {
    /// Not at that point (characters exist, or not at character select).
    No,
    Wait,
    TypeName,
    GiveUp(String),
}

/// The create screen opens a moment after an empty character list; the name goes in once; the game then lists it.
pub fn create_step(seen: &Seen, r: &Row, name: Option<&str>, typed_at: Option<u64>, now: u64) -> Create {
    let Some(ce) = seen.char_enum else { return Create::No };
    if r.chars > 0 {
        return Create::No;
    }
    let Some(_) = name else {
        return Create::GiveUp("no character yet and no name to make one: the game's create screen is left to the player".into());
    };
    // the create screen loads its models first (slower the first time, and with the game in the background)
    match typed_at {
        None if now.saturating_sub(ce) < CREATE_FIRST => Create::Wait,
        None => Create::TypeName,
        Some(_) if now.saturating_sub(ce) > CREATE_MAX => Create::GiveUp("the create screen didn't make the character".into()),
        Some(t) if now.saturating_sub(t) >= CREATE_RETRY => Create::TypeName,
        Some(_) => Create::Wait,
    }
}

/// The name goes in this long after the empty character list, again every [`CREATE_RETRY`], until [`CREATE_MAX`].
pub const CREATE_FIRST: u64 = 8_000;
pub const CREATE_RETRY: u64 = 10_000;
pub const CREATE_MAX: u64 = 60_000;

/// Runs until the character is in the world, the game closes, or a step times out. `name` is Quick start's name
/// for a first entry (None when the login has characters). `cover`: the sign-in cover's signals (crate::cover),
/// told how far it is, when the login went, and how it ended.
pub fn drive(p: &eng::Paths, account: &str, password: &str, name: Option<&str>, pid: u32, cover: Option<&crate::cover::Signals>) -> String {
    let start = dc::now_ms();
    let sql = match row_sql(account) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut game = dc::Process(pid);
    let mut d = Driver::new(start, false, false);
    let mut seen = Seen::default();
    let mut typed_at: Option<u64> = None;
    let mut log: Vec<(u64, String)> = Vec::new();
    let mut last = Row::default();
    let outcome = loop {
        let now = dc::now_ms();
        if !game.alive() {
            break "the game closed".to_string();
        }
        let r = eng::query(p, "acore_auth", &sql).ok().and_then(|o| parse_row(&o)).unwrap_or_default();
        last = r;
        seen.take(&r, start, now);
        if let Some(c) = cover {
            c.set_stage(stage(&seen));
        }
        match create_step(&seen, &r, name, typed_at, now) {
            Create::Wait => {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            Create::TypeName => {
                let ok = game.send(Key::Name, name.unwrap_or_default());
                log.push((now - start, format!("create screen: the name {} + Enter{}", name.unwrap_or_default(), if ok { "" } else { " (didn't reach the window)" })));
                typed_at = Some(now);
                continue;
            }
            Create::GiveUp(why) => break why,
            Create::No => {}
        }
        let win = game.window();
        match d.step(now, win, &seen.events()) {
            Act::Send(k) => {
                if !game.send(k, password) {
                    log.push((now - start, "the keys didn't reach the game window".into()));
                } else if k == Key::Password {
                    if let Some(c) = cover {
                        c.keys_sent();
                    }
                }
            }
            Act::Done => break "in the world".to_string(),
            Act::GiveUp(why) => break why,
            Act::Wait => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    let mut text = String::new();
    for (t, a) in d.log.iter().chain(log.iter()) {
        text.push_str(&format!("{t:>7} ms  {a}\n"));
    }
    text.push_str(&format!("outcome: {outcome}\n"));
    if let Some(c) = cover {
        c.finish(went_through(&outcome, &seen, last.chars, name));
    }
    let _ = std::fs::create_dir_all(&p.logs);
    let _ = std::fs::write(p.logs.join(LOG), text);
    outcome
}

/// Quick start on a stock game: the game's character select has no key that opens the create screen, so the new
/// character is made by the world itself. A fresh level-1 character of that race and class (the bots' own ready-made
/// characters, never one that is playing) is copied onto the player's login under the chosen name with the engine's
/// own `.pdump copy` (a new character; the bot's is left as it was). Then character select lists it and Enter World
/// goes in.
pub fn source_sql(race: u8, class: u8, female: bool, any_body: bool) -> String {
    let body = if any_body { String::new() } else { format!(" AND c.gender = {}", u8::from(female)) };
    format!(
        "SELECT c.name FROM acore_characters.characters c \
         WHERE c.race = {race} AND c.class = {class}{body} AND c.level = 1 AND c.online = 0 AND c.deleteDate IS NULL \
         AND c.account IN (SELECT account_id FROM acore_playerbots.playerbots_account_type) ORDER BY c.guid LIMIT 1"
    )
}

pub fn copy_command(from: &str, account: &str, name: &str) -> Result<String, String> {
    let plain = |s: &str| !s.is_empty() && s.len() <= 16 && s.chars().all(|c| c.is_ascii_alphanumeric());
    if !plain(from) || !plain(account) || !crate::hybrid::char_name_ok(name) {
        return Err("That character can't be copied.".into());
    }
    Ok(format!("pdump copy {from} {account} {name}"))
}

pub fn quick_copy(p: &eng::Paths, account: &str, name: &str, race: u8, class: u8, female: bool) -> Result<(), String> {
    let first = |sql: &str| eng::query(p, "acore_characters", sql).ok().and_then(|o| o.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string));
    let from = first(&source_sql(race, class, female, false))
        .or_else(|| first(&source_sql(race, class, female, true)))
        .ok_or("No ready-made character of that race and class to start from.")?;
    let said = eng::console(p, &copy_command(&from, account, name)?)?;
    let have = eng::chars_of(p, account).map(|c| c.iter().any(|x| x.name.eq_ignore_ascii_case(name))).unwrap_or(false);
    if !have {
        return Err(format!("The world didn't make {name}: {}", said.lines().next().unwrap_or("").chars().take(120).collect::<String>()));
    }
    Ok(())
}

/// Whether the launcher can type into the game here (Windows).
pub fn can_type() -> bool {
    cfg!(windows)
}

/// The last start's outcome line, for the page or a test.
pub fn last_outcome(logs: &Path) -> Option<String> {
    std::fs::read_to_string(logs.join(LOG)).ok()?.lines().rev().find_map(|l| l.strip_prefix("outcome: ").map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cover_follows_the_same_milestones() {
        use crate::cover::DenStage;
        let mut s = Seen::default();
        assert_eq!(stage(&s), DenStage::None);
        s.auth_ok = Some(5_000);
        assert_eq!(stage(&s), DenStage::Realm);
        s.char_enum = Some(7_000);
        assert_eq!(stage(&s), DenStage::CharSelect);
        s.in_world = Some(12_000);
        assert_eq!(stage(&s), DenStage::Entering);
        // how a drive went: in the world, or the player's own create screen (Skip: no name), else not
        let at_select = Seen { auth_ok: Some(1), char_enum: Some(2), in_world: None };
        assert!(went_through("in the world", &Seen::default(), 1, None));
        assert!(went_through("no character yet and no name to make one", &at_select, 0, None));
        assert!(!went_through("the create screen didn't make the character", &at_select, 0, Some("Thrall")));
        assert!(!went_through("the game closed", &Seen::default(), 0, None));
        assert!(!went_through("timed out", &at_select, 2, None));
    }

    #[test]
    fn follow_stops_in_the_world_or_at_character_select() {
        let start = 1_000;
        let mut s = Seen::default();
        assert_eq!(follow_done(&s, true, start, 5_000), None);
        s.auth_ok = Some(6_000);
        s.char_enum = Some(8_000);
        assert_eq!(follow_done(&s, true, start, 20_000), None, "a character is being entered: on to the world");
        assert_eq!(follow_done(&s, false, start, 8_500), None, "the list is drawing");
        assert_eq!(follow_done(&s, false, start, 8_000 + SELECT_GRACE), Some(true), "character select was the goal");
        s.in_world = Some(14_000);
        assert_eq!(follow_done(&s, true, start, 14_000), Some(true));
        assert_eq!(follow_done(&Seen::default(), true, start, start + FOLLOW_MAX + 1), Some(false), "gives up");
    }

    #[test]
    fn the_query_takes_only_a_plain_login() {
        assert!(row_sql("HKABC123").unwrap().contains("a.username = 'HKABC123'"));
        for bad in ["", "a'b", "lower", "TOOLONGNAME123456789"] {
            assert!(row_sql(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_row_parses_with_or_without_fractions() {
        assert_eq!(parse_row("1794476600\t1\t2\t0\n"), Some(Row { last_login: 1794476600, online: true, chars: 2, chars_online: false }));
        assert_eq!(parse_row("1794476600.000000\t0\t0\t0"), Some(Row { last_login: 1794476600, online: false, chars: 0, chars_online: false }));
        assert_eq!(parse_row(""), None);
        assert_eq!(parse_row("x\t1\t2\t0"), None);
    }

    #[test]
    fn milestones_follow_the_login_in_order_and_ignore_an_earlier_game() {
        let start = 1_000_000_000;
        let mut s = Seen::default();
        // an earlier login, before this start
        s.take(&Row { last_login: 990_000, online: true, chars: 1, chars_online: true }, start, start + 1);
        assert!(s.events().is_empty());
        s.take(&Row { last_login: 1_000_001, online: false, chars: 1, chars_online: false }, start, start + 5_000);
        assert_eq!(s.events().iter().map(|e| e.ev.as_str()).collect::<Vec<_>>(), ["auth_ok"]);
        s.take(&Row { last_login: 1_000_001, online: true, chars: 1, chars_online: false }, start, start + 7_000);
        s.take(&Row { last_login: 1_000_001, online: true, chars: 1, chars_online: true }, start, start + 12_000);
        let evs: Vec<String> = s.events().into_iter().map(|e| e.ev).collect();
        assert_eq!(evs, ["auth_ok", "world_auth", "char_enum", "in_world"]);
    }

    #[test]
    fn the_driver_types_the_password_then_enters_the_world() {
        let start = 1_000_000_000;
        let mut d = Driver::new(start, false, false);
        let mut s = Seen::default();
        // the window opens, the login screen settles, the password goes in
        assert_eq!(d.step(start + 1_000, true, &s.events()), Act::Wait);
        assert_eq!(d.step(start + 7_000, true, &s.events()), Act::Send(Key::Password));
        s.take(&Row { last_login: 1_000_008, online: true, chars: 1, chars_online: false }, start, start + 9_000);
        s.take(&Row { last_login: 1_000_008, online: true, chars: 1, chars_online: false }, start, start + 9_000);
        assert_eq!(d.step(start + 9_500, true, &s.events()), Act::Wait);
        assert_eq!(d.step(start + 12_000, true, &s.events()), Act::Send(Key::Enter));
        s.take(&Row { last_login: 1_000_008, online: true, chars: 1, chars_online: true }, start, start + 20_000);
        assert_eq!(d.step(start + 20_500, true, &s.events()), Act::Done);
    }

    #[test]
    fn quick_start_copies_a_fresh_ready_made_character_and_nothing_else() {
        let q = source_sql(1, 8, true, false);
        assert!(q.contains("c.race = 1 AND c.class = 8 AND c.gender = 1 AND c.level = 1 AND c.online = 0"));
        assert!(q.contains("playerbots_account_type") && q.contains("LIMIT 1"));
        assert!(!source_sql(1, 8, true, true).contains("gender"));
        assert_eq!(copy_command("Abelard", "HKABC", "Wren").unwrap(), "pdump copy Abelard HKABC Wren");
        for (f, a, n) in [("Ab;x", "HKABC", "Wren"), ("Abelard", "HK ABC", "Wren"), ("Abelard", "HKABC", "W'ren"), ("", "A", "Wren")] {
            assert!(copy_command(f, a, n).is_err(), "{f} {a} {n}");
        }
    }

    #[test]
    fn a_first_entry_types_the_name_once_then_waits_for_the_game() {
        let mut s = Seen::default();
        let r0 = Row { last_login: 10, online: true, chars: 0, chars_online: false };
        s.auth_ok = Some(100);
        s.char_enum = Some(1_000);
        assert_eq!(create_step(&s, &r0, Some("Wren"), None, 5_000), Create::Wait);
        assert_eq!(create_step(&s, &r0, Some("Wren"), None, 9_500), Create::TypeName);
        assert_eq!(create_step(&s, &r0, Some("Wren"), Some(9_500), 15_000), Create::Wait);
        // not made yet: the name again
        assert_eq!(create_step(&s, &r0, Some("Wren"), Some(9_500), 19_600), Create::TypeName);
        assert!(matches!(create_step(&s, &r0, Some("Wren"), Some(55_000), 62_000), Create::GiveUp(_)));
        // made: the driver goes on (Enter World)
        assert_eq!(create_step(&s, &Row { chars: 1, ..r0 }, Some("Wren"), Some(9_500), 12_000), Create::No);
        // no name: left to the player
        assert!(matches!(create_step(&s, &r0, None, None, 9_000), Create::GiveUp(_)));
        // not at character select yet
        assert_eq!(create_step(&Seen::default(), &r0, Some("Wren"), None, 9_000), Create::No);
    }
}
