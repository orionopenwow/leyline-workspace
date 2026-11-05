//! Hosting friends from Your Den with no Docker (owner 4 Nov: "easy Den hosting for friends ... one click"; the
//! guide is docs/DEN-HOSTING.md). The relay tunnel, the invite link and each friend's login were already here
//! (guests.rs, the launcher's host/litenet.rs and host/denways.rs); this file is what the host sees and does while
//! friends are in:
//!
//! * **Who's in**: `<dens>/den-lite.online` ([`ONLINE_FILE`]), rewritten within a second of a change: everyone in
//!   the world now (the host and each friend: their character, their login's account, whether they may build, whether
//!   they are kept, since when). The launcher's Friends list reads it. Never a password or an address.
//! * **The host's asks**: `<dens>/den-lite.host-ask` ([`HOST_ASK_FILE`]), lines the launcher writes and Den Lite
//!   takes once read (as `den-lite.direct-want`): `kick <account|name>` (their login taken back and their game sent
//!   out of the Den at once), `build <account|name> on|off` (that friend's own build right, kept in guests.json
//!   across restarts), `build all on|off` (the Den-wide switch, `.den friends build on`), `keep <account|name>
//!   on|off` (their login lasts a week from their last visit). The file is in the Dens folder on the host's own PC:
//!   a friend's game can't write it.
//! * **Build rights per friend**: [`may_build`] is what world.rs asks: the host, anyone while the Den-wide switch is
//!   on, or a friend whose own Build switch is on.
//! * **Kept friends**: while a kept friend is in, their login is renewed (guests.rs `renew`), so the link they have
//!   keeps working week after week.
//!
//! The tournament ticker (tourney.rs) runs on the same thread: one Den Lite thread for everything the host's friends
//! see happen by itself.

use crate::Shared;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const ONLINE_FILE: &str = "den-lite.online";
pub const HOST_ASK_FILE: &str = "den-lite.host-ask";
/// How often the ticker looks (the asks, who's in, the tournament's countdown).
pub const TICK_MS: u64 = 250;
/// guests.json is read again this often for the build rights (a launcher's `den-lite guest build` changes it too).
const BUILDERS_EVERY_MS: u64 = 3000;

/// The friends (guids) whose own Build switch is on, as guests.json said at the last look.
static BUILDERS: Mutex<Option<HashSet<u64>>> = Mutex::new(None);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// guests.json read again for the build rights.
pub fn refresh_builders(dir: &Path, now: u64) {
    let set: HashSet<u64> = crate::guests::list(dir).into_iter().filter(|g| g.live(now) && g.build).map(|g| g.guid).collect();
    *BUILDERS.lock().unwrap_or_else(|e| e.into_inner()) = Some(set);
}

/// Whether this character may build here: the host, everyone while `.den friends build on`, or a friend whose own
/// Build switch is on.
pub fn may_build(shared: &Shared, me: &crate::world::Me) -> bool {
    if me.host || shared.guests_build.load(Ordering::Relaxed) {
        return true;
    }
    let mut b = BUILDERS.lock().unwrap_or_else(|e| e.into_inner());
    if b.is_none() {
        drop(b);
        refresh_builders(&shared.cfg.den_dir, now());
        b = BUILDERS.lock().unwrap_or_else(|e| e.into_inner());
    }
    b.as_ref().is_some_and(|s| s.contains(&me.guid))
}

/// One of the host's asks.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    Kick(String),
    Build(String, bool),
    BuildAll(bool),
    Keep(String, bool),
}

/// The asks file's lines (anything else is skipped; at most 32 a look).
pub fn parse_asks(text: &str) -> Vec<Ask> {
    let ok = |w: &str| (1..=16).contains(&w.len()) && w.chars().all(|c| c.is_ascii_alphanumeric());
    let mut out = Vec::new();
    for l in text.lines().take(32) {
        let w: Vec<&str> = l.split_whitespace().collect();
        let on = |i: usize| match w.get(i).copied() {
            Some("on") => Some(true),
            Some("off") => Some(false),
            _ => None,
        };
        match (w.first().copied(), w.get(1).copied()) {
            (Some("kick"), Some(who)) if ok(who) && w.len() == 2 => out.push(Ask::Kick(who.into())),
            (Some("build"), Some("all")) => {
                if let Some(v) = on(2) {
                    out.push(Ask::BuildAll(v))
                }
            }
            (Some("build"), Some(who)) if ok(who) => {
                if let Some(v) = on(2) {
                    out.push(Ask::Build(who.into(), v))
                }
            }
            (Some("keep"), Some(who)) if ok(who) => {
                if let Some(v) = on(2) {
                    out.push(Ask::Keep(who.into(), v))
                }
            }
            _ => {}
        }
    }
    out
}

/// One player in the world as the Friends list shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct Here {
    pub name: String,
    pub guid: u64,
    pub host: bool,
    pub since: u64,
}

/// `<dens>/den-lite.online`'s text: who is in, with each friend's login switches (no secrets).
pub fn online_doc(here: &[Here], guests: &[crate::guests::Guest], all_build: bool, now: u64) -> Value {
    let players: Vec<Value> = here
        .iter()
        .map(|p| {
            let g = guests.iter().find(|g| g.guid == p.guid && !p.host);
            json!({
                "name": p.name,
                "host": p.host,
                "user": g.map(|g| g.user.as_str()).unwrap_or(""),
                "builds": p.host || all_build || g.is_some_and(|g| g.build),
                "keep": g.is_some_and(|g| g.keep),
                "since": p.since,
            })
        })
        .collect();
    json!({ "at": now, "all_build": all_build, "players": players })
}

/// When each session came in (by session id), for "since".
static SINCE: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());

fn here_now(shared: &Shared, now: u64) -> Vec<Here> {
    let ps = shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut since = SINCE.lock().unwrap_or_else(|e| e.into_inner());
    since.retain(|(s, _)| ps.iter().any(|p| p.sess == *s));
    ps.iter()
        .map(|p| {
            let t = match since.iter().find(|(s, _)| *s == p.sess) {
                Some((_, t)) => *t,
                None => {
                    since.push((p.sess, now));
                    now
                }
            };
            Here { name: p.me.name.clone(), guid: p.me.guid, host: p.me.host, since: t }
        })
        .collect()
}

/// The ask carried out. The line for the host (and the events).
pub fn apply(shared: &Shared, ask: &Ask) -> String {
    let dir = &shared.cfg.den_dir;
    let t = now();
    let line = match ask {
        Ask::Kick(who) => {
            let g = crate::guests::list(dir).into_iter().find(|g| g.live(t) && (g.user.eq_ignore_ascii_case(who) || g.name.eq_ignore_ascii_case(who)));
            match g {
                None => format!("No friend called {who} has a login that works now."),
                Some(g) => {
                    let _ = crate::guests::revoke(dir, &g.user, t);
                    let peers: Vec<crate::world::Peer> =
                        shared.players.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|p| !p.me.host && p.me.guid == g.guid).cloned().collect();
                    for p in &peers {
                        let _ = crate::world::say_to(&p.w, "Your host has closed their Den to you. Thanks for visiting!");
                        p.w.close();
                    }
                    crate::tourney::left(shared, &g.name);
                    shared.event("guest_kicked", &g.name, if peers.is_empty() { "login" } else { "login and game" });
                    format!("{} is out of your Den, and their link no longer works.", g.name)
                }
            }
        }
        Ask::Build(who, on) => match crate::guests::set(dir, who, "build", *on, t) {
            Ok(g) => {
                refresh_builders(dir, t);
                tell_guest(shared, g.guid, if *on { "Your host lets you build in their Den now." } else { "Your host builds here now; you can still look around." });
                shared.event("guest_build", &g.name, if *on { "on" } else { "off" });
                format!("{} {} build now.", g.name, if *on { "can" } else { "can't" })
            }
            Err(e) => e,
        },
        Ask::BuildAll(on) => {
            shared.guests_build.store(*on, Ordering::Relaxed);
            for p in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                let _ = crate::world::say_to(&p.w, if *on { "Your friends can build in your Den now." } else { "Only you build in your Den now." });
            }
            shared.event("guest_build", "all", if *on { "on" } else { "off" });
            if *on { "Every friend can build now.".into() } else { "Only you (and friends with their own Build on) build now.".into() }
        }
        Ask::Keep(who, on) => match crate::guests::set(dir, who, "keep", *on, t) {
            Ok(g) => format!("{} {}", g.name, if *on { "is kept: their link works for a week from their last visit." } else { "is no longer kept." }),
            Err(e) => e,
        },
    };
    let _ = std::fs::write(dir.join(format!("{HOST_ASK_FILE}.done")), format!("{}\n", line));
    line
}

fn tell_guest(shared: &Shared, guid: u64, text: &str) {
    for p in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        if !p.me.host && p.me.guid == guid {
            let _ = crate::world::say_to(&p.w, text);
        }
    }
}

/// The host's asks, taken once read.
fn take_asks(shared: &Shared) -> Vec<Ask> {
    let p = shared.cfg.den_dir.join(HOST_ASK_FILE);
    let Ok(t) = std::fs::read_to_string(&p) else { return Vec::new() };
    let _ = std::fs::remove_file(&p);
    if t.len() > 4096 {
        return Vec::new();
    }
    parse_asks(&t)
}

/// The ticker (lib.rs starts it): the host's asks, who's in, kept friends renewed, the tournament.
pub fn run(shared: Arc<Shared>) {
    let mut last_online = String::new();
    let mut builders_at = 0u64;
    loop {
        std::thread::sleep(Duration::from_millis(TICK_MS));
        let (t, ms) = (now(), now_ms());
        for a in take_asks(&shared) {
            let line = apply(&shared, &a);
            shared.log(&format!("host ask: {line}"));
        }
        if ms.saturating_sub(builders_at) >= BUILDERS_EVERY_MS {
            builders_at = ms;
            refresh_builders(&shared.cfg.den_dir, t);
        }
        let here = here_now(&shared, t);
        let guests = crate::guests::list(&shared.cfg.den_dir);
        for h in here.iter().filter(|h| !h.host) {
            crate::guests::renew(&shared.cfg.den_dir, h.guid, t);
        }
        let mut doc = online_doc(&here, &guests, shared.guests_build.load(Ordering::Relaxed), t);
        // "at" changes every second: compare without it, write when who's in changed (or once a minute, so the
        // launcher can tell a Den that stopped from one with nobody in)
        let key = doc["players"].to_string() + &doc["all_build"].to_string();
        if key != last_online || t % 60 == 0 {
            last_online = key;
            if let Some(o) = doc.as_object_mut() {
                o.insert("tourney".into(), crate::tourney::summary_json());
            }
            let p = shared.cfg.den_dir.join(ONLINE_FILE);
            let tmp = shared.cfg.den_dir.join(format!(".{ONLINE_FILE}.tmp"));
            if std::fs::write(&tmp, doc.to_string()).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
        crate::tourney::tick(&shared, ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guests::Guest;

    fn g(user: &str, name: &str, guid: u64, build: bool, keep: bool) -> Guest {
        Guest { user: user.into(), name: name.into(), salt: String::new(), verifier: String::new(), made: 1, expires: 99, revoked: false, guid, build, keep }
    }

    #[test]
    fn the_hosts_asks_are_read_strictly() {
        let a = parse_asks("kick GABCDE2345\nbuild rowan on\nbuild all off\nkeep Briar on\nbuild x maybe\nkick ../etc\nrm -rf /\nkick a b\n");
        assert_eq!(a, vec![Ask::Kick("GABCDE2345".into()), Ask::Build("rowan".into(), true), Ask::BuildAll(false), Ask::Keep("Briar".into(), true)]);
        assert!(parse_asks("").is_empty());
        assert_eq!(parse_asks(&"kick A\n".repeat(100)).len(), 32, "at most 32 a look");
    }

    #[test]
    fn whos_in_without_secrets() {
        let here = vec![
            Here { name: "Wren".into(), guid: 1, host: true, since: 10 },
            Here { name: "Rowan".into(), guid: 100, host: false, since: 20 },
            Here { name: "Briar".into(), guid: 101, host: false, since: 30 },
        ];
        let guests = vec![g("GROWAN0001", "Rowan", 100, true, false), g("GBRIAR0001", "Briar", 101, false, true)];
        let d = online_doc(&here, &guests, false, 40);
        let p = d["players"].as_array().unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!((p[0]["host"].as_bool(), p[0]["builds"].as_bool(), p[0]["user"].as_str()), (Some(true), Some(true), Some("")));
        assert_eq!((p[1]["user"].as_str(), p[1]["builds"].as_bool(), p[1]["keep"].as_bool()), (Some("GROWAN0001"), Some(true), Some(false)));
        assert_eq!((p[2]["builds"].as_bool(), p[2]["keep"].as_bool(), p[2]["since"].as_u64()), (Some(false), Some(true), Some(30)));
        let all = online_doc(&here, &guests, true, 40);
        assert_eq!(all["players"][2]["builds"].as_bool(), Some(true), "the Den-wide switch");
        let text = d.to_string();
        assert!(!text.contains("verifier") && !text.contains("salt") && !text.contains("pass"));
    }
}
