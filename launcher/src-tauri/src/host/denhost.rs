//! Hosting friends in Your Den with no Docker (owner 4 Nov: "easy Den hosting for friends ... one click"; the guide
//! is docs/DEN-HOSTING.md). Den Lite is reached through the Leyline relay (host/litenet.rs: no port forwarding, no
//! home address shown), each friend gets a login of their own (labs/den-lite guests.rs) and this file is the rest:
//!
//! * **Who's in**: Den Lite writes `<dens>/den-lite.online` within a second of a change (labs/den-lite hosting.rs):
//!   [`read_online`] reads it for the Friends list (a file older than [`STALE_SECS`] is a Den that stopped).
//! * **The host's asks**: kick, build on or off for one friend or all, keep: one line each in
//!   `<dens>/den-lite.host-ask` ([`ask_line`] checks them), which Den Lite takes within a quarter second. Build and
//!   keep are also set with `den-lite guest build|keep` (host/lite.rs) so they hold while Den Lite is stopped.
//! * **An invite code** for pasting where a link is awkward ([`code_from_link`] / [`link_from_code`]):
//!   `DEN-<realm>-<knock>-<account>-<password>`, the same secrets as the link (the relay way only: the same network
//!   and own address ways stay in the link), any case.
//! * **Sharing across restarts**: `<den-lite root>/state/den-sharing` is there while the host shares ([`set_sharing`]);
//!   when Leyline starts again with it there, Den Lite and the tunnel come back by themselves (app.rs
//!   `den_share_resume`), so friends' links keep working after the host's PC restarts.
//!
//! Plain std and serde only, unit tested below.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const ONLINE_FILE: &str = "den-lite.online";
pub const HOST_ASK_FILE: &str = "den-lite.host-ask";
pub const SHARING_FILE: &str = "den-sharing";
/// Den Lite rewrites the online file at least once a minute while it runs.
pub const STALE_SECS: u64 = 90;

/// One player in the Den now.
#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize, Default)]
pub struct Here {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub host: bool,
    /// A friend's login (their account), "" for the host.
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub builds: bool,
    #[serde(default)]
    pub keep: bool,
    #[serde(default)]
    pub since: u64,
}

/// What the Friends list shows.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Online {
    /// Den Lite wrote the file lately (it runs).
    pub fresh: bool,
    pub players: Vec<Here>,
    pub all_build: bool,
    /// The tournament under way, when there is one (labs/den-lite tourney.rs summary_json).
    pub tourney: Option<Value>,
}

fn clean(s: &str, n: usize) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).take(n).collect()
}

/// The online file's text, checked: names and accounts letters and digits only, at most 40 players.
pub fn parse_online(text: &str, now: u64) -> Online {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Online::default() };
    let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
    let players = v
        .get("players")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .take(40)
                .filter_map(|p| serde_json::from_value::<Here>(p.clone()).ok())
                .map(|mut h| {
                    h.name = clean(&h.name, 12);
                    h.user = clean(&h.user, 16);
                    h
                })
                .filter(|h| !h.name.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let tourney = v.get("tourney").filter(|t| t.is_object()).cloned();
    Online { fresh: now.saturating_sub(at) <= STALE_SECS, players, all_build: v.get("all_build").and_then(Value::as_bool).unwrap_or(false), tourney }
}

pub fn read_online(dens: &Path, now: u64) -> Online {
    match std::fs::read_to_string(dens.join(ONLINE_FILE)) {
        Ok(t) if t.len() < 64 * 1024 => parse_online(&t, now),
        _ => Online::default(),
    }
}

/// One ask for Den Lite: `kick`, `build`, `keep` (`on` for build and keep), `who` a friend's account or character
/// name, or `all` for build. None: not an ask this file sends.
pub fn ask_line(action: &str, who: &str, on: bool) -> Option<String> {
    let ok = (1..=16).contains(&who.len()) && who.chars().all(|c| c.is_ascii_alphanumeric());
    let v = if on { "on" } else { "off" };
    match action {
        "kick" if ok && who != "all" => Some(format!("kick {who}")),
        "build" if ok => Some(format!("build {who} {v}")),
        "keep" if ok && who != "all" => Some(format!("keep {who} {v}")),
        _ => None,
    }
}

/// The ask written for Den Lite (added to any it hasn't read yet).
pub fn ask(dens: &Path, line: &str) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(dens).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(dens.join(HOST_ASK_FILE)).map_err(|e| format!("Couldn't ask your Den's server: {e}"))?;
    writeln!(f, "{line}").map_err(|e| format!("Couldn't ask your Den's server: {e}"))
}

/// What Den Lite said about the last ask (`<dens>/den-lite.host-ask.done`), within `wait`; None: it didn't read it
/// (Den Lite isn't running, or is from before this).
pub fn answer(dens: &Path, wait: std::time::Duration) -> Option<String> {
    let done = dens.join(format!("{HOST_ASK_FILE}.done"));
    let ask = dens.join(HOST_ASK_FILE);
    let t0 = std::time::Instant::now();
    while t0.elapsed() < wait {
        if !ask.exists() {
            return std::fs::read_to_string(&done).ok().map(|t| t.trim().chars().take(200).collect());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    None
}

// ------------------------------------------------------------------------------------------------ the invite code

fn relay_id_ok(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}
fn plain_upper(s: &str, n: std::ops::RangeInclusive<usize>) -> bool {
    n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// The code for a Den invite link: `DEN-<realm id>-<knock>-<account>-<password>` (the relay way).
pub fn code_from_link(link: &str) -> Option<String> {
    let (_, rid, secret) = crate::realms::parse_invite(link)?;
    let (u, p) = crate::host::litenet::guest_in(link)?;
    Some(format!("DEN-{}-{}-{}-{}", rid.to_ascii_uppercase(), secret, u, p))
}

/// The link for a code (any case, spaces and line breaks taken out), on `relay`. None: not a Den code.
pub fn link_from_code(code: &str, relay: &str) -> Option<String> {
    let c: String = code.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_uppercase();
    let parts: Vec<&str> = c.split('-').collect();
    let [den, rid, secret, user, pass] = parts.as_slice() else { return None };
    let rid = rid.to_ascii_lowercase();
    if *den != "DEN" || !relay_id_ok(&rid) || !plain_upper(secret, 16..=40) || !user.starts_with('G') || !plain_upper(user, 4..=16) || !plain_upper(pass, 8..=16) {
        return None;
    }
    Some(crate::host::litenet::invite_link(relay, &rid, secret, Some((*user, *pass))))
}

// ------------------------------------------------------------------------------------------------ sharing kept

pub fn sharing_path(root: &Path) -> PathBuf {
    root.join("state").join(SHARING_FILE)
}

/// Sharing on (Invite friends) or off (Stop sharing), kept for the next start of Leyline.
pub fn set_sharing(root: &Path, on: bool) {
    let p = sharing_path(root);
    if on {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&p, "on\n");
    } else {
        let _ = std::fs::remove_file(&p);
    }
}

pub fn sharing(root: &Path) -> bool {
    sharing_path(root).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = "https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789";

    #[test]
    fn the_code_and_the_link_carry_the_same_way_in() {
        let c = code_from_link(LINK).unwrap();
        assert_eq!(c, "DEN-KZDVVJ2UMNDUYAUF-ABCDEFGHJKLMNPQRSTUVWXYZ-GABCDE2345-ABCDEFGH23456789");
        assert_eq!(link_from_code(&c, "relay.openvanilla.app").as_deref(), Some(LINK));
        assert_eq!(link_from_code(&format!("  {}\n", c.to_lowercase()), "relay.openvanilla.app").as_deref(), Some(LINK), "any case, spaces out");
        // the launcher's own parsers take what it makes
        let back = link_from_code(&c, "relay.openvanilla.app").unwrap();
        assert!(crate::realms::parse_invite(&back).is_some() && crate::host::litenet::guest_in(&back).is_some());
        for bad in ["", "DEN-x", "REALM-KZDVVJ2UMNDUYAUF-ABCDEFGHJKLMNPQRSTUVWXYZ-GABCDE2345-ABCDEFGH23456789",
            "DEN-KZDVVJ2UMNDUYAU1-ABCDEFGHJKLMNPQRSTUVWXYZ-GABCDE2345-ABCDEFGH23456789", "DEN-KZDVVJ2UMNDUYAUF-ABC-GABCDE2345-ABCDEFGH23456789",
            "DEN-KZDVVJ2UMNDUYAUF-ABCDEFGHJKLMNPQRSTUVWXYZ-XABCDE2345-ABCDEFGH23456789"] {
            assert_eq!(link_from_code(bad, "relay.openvanilla.app"), None, "{bad}");
        }
        assert_eq!(code_from_link("https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ"), None, "a realm's invite has no friend's login");
    }

    #[test]
    fn whos_in_read_carefully() {
        let t = r#"{"at":1000,"all_build":false,"players":[{"name":"Wren","host":true,"user":"","builds":true,"keep":false,"since":900},
            {"name":"Rowan<b>","host":false,"user":"GABCDE2345","builds":false,"keep":true,"since":950},{"nope":1}],"tourney":{"title":"Cup","phase":"fight"}}"#;
        let o = parse_online(t, 1030);
        assert!(o.fresh);
        assert_eq!(o.players.len(), 2, "a row without a name is skipped");
        assert_eq!((o.players[1].name.as_str(), o.players[1].user.as_str(), o.players[1].keep), ("Rowanb", "GABCDE2345", true));
        assert_eq!(o.tourney.as_ref().and_then(|t| t["phase"].as_str()), Some("fight"));
        assert!(!parse_online(t, 1000 + STALE_SECS + 1).fresh, "an old file: the Den stopped");
        assert_eq!(parse_online("junk", 1), Online::default());
    }

    #[test]
    fn asks_and_the_sharing_mark() {
        assert_eq!(ask_line("kick", "GABCDE2345", true).as_deref(), Some("kick GABCDE2345"));
        assert_eq!(ask_line("build", "Rowan", false).as_deref(), Some("build Rowan off"));
        assert_eq!(ask_line("build", "all", true).as_deref(), Some("build all on"));
        assert_eq!(ask_line("kick", "all", true), None, "never everyone at once by accident");
        assert_eq!(ask_line("kick", "a b", true), None);
        assert_eq!(ask_line("rm", "x", true), None);
        let d = tempfile::tempdir().unwrap();
        ask(d.path(), "kick GABCDE2345").unwrap();
        ask(d.path(), "build Rowan on").unwrap();
        assert_eq!(std::fs::read_to_string(d.path().join(HOST_ASK_FILE)).unwrap(), "kick GABCDE2345\nbuild Rowan on\n");
        assert_eq!(answer(d.path(), std::time::Duration::from_millis(150)), None, "not read: Den Lite isn't running");
        std::fs::remove_file(d.path().join(HOST_ASK_FILE)).unwrap();
        std::fs::write(d.path().join(format!("{HOST_ASK_FILE}.done")), "Rowan can build now.\n").unwrap();
        assert_eq!(answer(d.path(), std::time::Duration::from_millis(150)).as_deref(), Some("Rowan can build now."));
        assert!(!sharing(d.path()));
        set_sharing(d.path(), true);
        assert!(sharing(d.path()));
        set_sharing(d.path(), false);
        assert!(!sharing(d.path()));
    }
}
