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

use std::path::{Path, PathBuf};

/// In the launcher program, so the first-run tool can tell a launcher that has `play-test` from an older one.
pub const CLI_MARK: &str = "leyline-play-test-v1";
/// The test account's Hero Key, from the first-run tool (tools/audit's test account), only in the environment.
pub const KEY_ENV: &str = "LEYLINE_TEST_HEROKEY";
/// `<data folder>/play-test.json`: the run's report (the first-run tool reads it).
pub const REPORT: &str = "play-test.json";
/// The longest `--hold`: the game stays open this many seconds at most after it got there.
pub const HOLD_MAX: u64 = 600;

#[derive(Debug, Clone, PartialEq)]
pub struct Cli {
    /// PLAY on the official realm (today the only realm the test plays).
    pub official: bool,
    /// A new, empty data folder for this run.
    pub fresh: bool,
    /// The game folder to add (a fresh data folder has none).
    pub game: Option<PathBuf>,
    /// Enter the world, not only character select.
    pub world: bool,
    /// Seconds the game stays open after it got there (for the screenshot), then it is closed.
    pub hold: u64,
}

impl Default for Cli {
    fn default() -> Self {
        Cli { official: false, fresh: false, game: None, world: false, hold: 30 }
    }
}

/// `Leyline play-test [--official] [--fresh] [--game <folder>] [--world] [--hold <s>]`. None when the first
/// argument isn't `play-test`; Err for an argument it doesn't know (a key or a password on the command line among
/// them: those go through [`KEY_ENV`] only).
pub fn parse_cli(args: &[String]) -> Option<Result<Cli, String>> {
    if args.get(1).map(String::as_str) != Some("play-test") {
        return None;
    }
    let mut c = Cli::default();
    let mut it = args.iter().skip(2);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--official" => c.official = true,
            "--fresh" => c.fresh = true,
            "--world" => c.world = true,
            "--game" => match it.next() {
                Some(g) if !g.trim().is_empty() => c.game = Some(PathBuf::from(g.trim())),
                _ => return Some(Err("--game needs the game folder".into())),
            },
            "--hold" => c.hold = it.next().and_then(|h| h.parse().ok()).unwrap_or(30).min(HOLD_MAX),
            other => return Some(Err(format!("play-test doesn't take {other:?}"))),
        }
    }
    if !c.official {
        return Some(Err("play-test plays the official realm: add --official".into()));
    }
    Some(Ok(c))
}

/// The fresh data folder for a run started at `now_ms`, under `base` (the system's temp folder).
pub fn fresh_dir(base: &Path, now_ms: u64) -> PathBuf {
    base.join(format!("leyline-first-run-{now_ms}"))
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
    fn fresh_dir_is_new_per_run() {
        let base = Path::new("/tmp");
        assert_ne!(fresh_dir(base, 1), fresh_dir(base, 2));
        assert!(fresh_dir(base, 1).starts_with(base));
    }
}
