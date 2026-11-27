//! Leyline's public addresses: the one place in the launcher (docs/DOMAIN-MOVE.md; the same list is
//! `tools/domain/addresses.json`, which `tools/domain/tests/test_domain.py` checks this file against).
//!
//! Every web address the launcher calls or opens is built from these with `site!`, `relay!`. The new names come first;
//! the old ones keep answering forever, so a request that cannot reach a new name (no DNS yet, a network that blocks
//! it) is tried once more on the old one (`fallback`, used by `net`, `social`, `bugs` and `rankings`). Downloads
//! are accepted from the site on either name (`same_site`). The release check (GitHub) and deep links do not change.

/// The server's own features: off (owner, 26 Nov 2026: Leyline's only server is shut down for good; the launcher is a
/// launcher aggregator, docs/LAUNCHER-LITE.md). The website comes back as a static site on GitHub Pages at the same
/// address, so only its read-only files are fetched (`off`); the browser game (`GAME_HOST`), the relay (`RELAY_HOST`)
/// and every old name are never called, and nothing is posted to the site. The code stays behind this one switch so
/// it could come back: `true` is the launcher as it was. The window has the same switch (ui/lite.js).
pub const SERVER_FEATURES: bool = false;

/// The website (the old name: `OLD_SITE_HOST`).
pub const SITE_HOST: &str = "leylinelauncher.com";
/// The browser game.
pub const GAME_HOST: &str = "play.leylinelauncher.com";
/// The Leyline network relay.
pub const RELAY_HOST: &str = "relay.leylinelauncher.com";
pub const OLD_SITE_HOST: &str = "play.openvanilla.app";
pub const OLD_GAME_HOST: &str = "wrath.openvanilla.app";
pub const OLD_RELAY_HOST: &str = "relay.openvanilla.app";

/// `site!("/wrath/x")`: the full https address of a page or API on the website (new name).
#[macro_export]
macro_rules! site {
    ($path:literal) => {
        concat!("https://leylinelauncher.com", $path)
    };
}
/// `relay!("/api/x")`: the same on the relay.
#[macro_export]
macro_rules! relay {
    ($path:literal) => {
        concat!("https://relay.leylinelauncher.com", $path)
    };
}

/// The website's origin.
pub const SITE: &str = site!("");
/// Both names of each part, new first.
pub const PAIRS: &[(&str, &str)] = &[(SITE_HOST, OLD_SITE_HOST), (GAME_HOST, OLD_GAME_HOST), (RELAY_HOST, OLD_RELAY_HOST)];

fn host_of(url: &str) -> Option<(&str, &str, &str)> {
    // (scheme incl. "://", host, rest)
    let i = url.find("://")? + 3;
    let rest = &url[i..];
    let end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
    Some((&url[..i], &rest[..end], &rest[end..]))
}

/// The same address on the old name, when `url` is on one of the new names (else None).
pub fn fallback(url: &str) -> Option<String> {
    let (scheme, host, rest) = host_of(url)?;
    let old = PAIRS.iter().find(|(n, _)| host.eq_ignore_ascii_case(n))?.1;
    Some(format!("{scheme}{old}{rest}"))
}

/// The name players read for an old one (`relay.openvanilla.app` -> `relay.leylinelauncher.com`); others unchanged.
pub fn current(host: &str) -> &str {
    PAIRS.iter().find(|(_, o)| host.eq_ignore_ascii_case(o)).map(|(n, _)| *n).unwrap_or(host)
}

/// True for either name of the website's origin (`https://leylinelauncher.com`, `https://play.openvanilla.app`).
pub fn is_site_origin(origin: &str) -> bool {
    origin == SITE || origin.strip_prefix("https://") == Some(OLD_SITE_HOST)
}

/// Two origins that are the same site: equal, or both names of the Leyline website (a catalog fetched on the old
/// name lists its downloads there; one fetched on the new name, there).
pub fn same_site(a: &str, b: &str) -> bool {
    a == b || (is_site_origin(a) && is_site_origin(b))
}

/// The path of `url` when it is on `origin`'s site (either name of the Leyline website counts as the same site),
/// else None: `on_site("https://play.openvanilla.app/x", "https://leylinelauncher.com") == Some("/x")`.
pub fn site_path<'a>(url: &'a str, origin: &str) -> Option<&'a str> {
    let o = crate::net::origin(url)?;
    let rest = url.get(o.len()..)?;
    (same_site(&o, origin) && rest.starts_with('/')).then_some(rest)
}

/// Either name of the official realm (the desktop realmlist): `leylinelauncher.com` or `play.openvanilla.app`.
pub fn is_official_realm(addr: &str) -> bool {
    let h = addr.trim().trim_end_matches(":3724");
    h.eq_ignore_ascii_case(SITE_HOST) || h.eq_ignore_ascii_case(OLD_SITE_HOST)
}

/// The realmlist to write for the official realm: the new name when this computer can resolve it, else the old one
/// (both reach the same server). Looked up once per run.
pub fn official_realm() -> &'static str {
    use std::net::ToSocketAddrs;
    static PICK: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    PICK.get_or_init(|| {
        if std::env::var_os("LEYLINE_OLD_NAMES").is_some() {
            return OLD_SITE_HOST;
        }
        if (SITE_HOST, 3724).to_socket_addrs().is_ok_and(|mut a| a.next().is_some()) {
            SITE_HOST
        } else {
            OLD_SITE_HOST
        }
    })
}

/// The hosts that are gone with the server (`SERVER_FEATURES` off): the browser game and the relay on both names, and
/// the website's old name. The website itself stays (static, read-only).
pub fn dead_host(host: &str) -> bool {
    let h = host.trim().trim_end_matches('.');
    !SERVER_FEATURES
        && [GAME_HOST, RELAY_HOST, OLD_SITE_HOST, OLD_GAME_HOST, OLD_RELAY_HOST].iter().any(|d| h.eq_ignore_ascii_case(d))
}

/// A path on the static website that only the old server answered: its APIs (`/api/`, `/wrath/api/…`,
/// `/wrath/forum/api`, `/wrath/bugs/api`), sign-in (`/auth/`, `/account`) and the official realm's live status.
pub fn dynamic_site_path(path: &str) -> bool {
    let p = path.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    p.contains("/api/") || p.ends_with("/api") || p.starts_with("/auth/") || p.starts_with("/account") || p == "/wrath/status.json"
}

/// True when `url` must not be called with `SERVER_FEATURES` off: a dead host (`dead_host`), or the website's
/// dynamic parts (`dynamic_site_path`). Answered without touching the network.
pub fn off(url: &str) -> bool {
    if SERVER_FEATURES {
        return false;
    }
    let Some((_, host, rest)) = host_of(url) else { return false };
    if dead_host(host) {
        return true;
    }
    if host.eq_ignore_ascii_case(SITE_HOST) {
        // the path after an optional :port
        let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
        return dynamic_site_path(path);
    }
    false
}

/// What a call to an `off` address says (an error made without the network: ureq refuses the scheme at once).
pub const OFF_MESSAGE: &str = "Leyline no longer has a server of its own, so this isn't available.";
fn off_error() -> ureq::Error {
    ureq::get("leyline-off://server-features-off").call().err().expect("an unknown scheme is refused")
}

/// Run `call` on `url`; when it could not reach the server at all (DNS, connect, TLS: not an HTTP answer) and
/// `url` is on a new name, run it once more on the old name. Once a new name has answered in this run, this computer
/// resolves it: a later failure is the server or the network, and the old name (the same server) would only double the
/// wait (audit 12 Nov: a realm list could take 32 s), so it is not tried again.
pub fn try_both<T>(url: &str, mut call: impl FnMut(&str) -> Result<T, ureq::Error>) -> Result<T, ureq::Error> {
    if off(url) {
        return Err(off_error());
    }
    match call(url) {
        Err(ureq::Error::Transport(t)) => match fallback(url).filter(|o| !new_name_answered() && !off(o)) {
            Some(old) => call(&old),
            None => Err(ureq::Error::Transport(t)),
        },
        other => {
            if fallback(url).is_some() {
                set_new_name_answered();
            }
            other
        }
    }
}

// One flag for the whole run (every request thread); each test thread has its own, so tests don't meet.
#[cfg(not(test))]
static NEW_NAME_ANSWERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(not(test))]
fn new_name_answered() -> bool {
    NEW_NAME_ANSWERED.load(std::sync::atomic::Ordering::Relaxed)
}
#[cfg(not(test))]
fn set_new_name_answered() {
    NEW_NAME_ANSWERED.store(true, std::sync::atomic::Ordering::Relaxed)
}
#[cfg(test)]
thread_local! { static NEW_NAME_ANSWERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
fn new_name_answered() -> bool {
    NEW_NAME_ANSWERED.with(|c| c.get())
}
#[cfg(test)]
fn set_new_name_answered() {
    NEW_NAME_ANSWERED.with(|c| c.set(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names() {
        assert_eq!(SITE, "https://leylinelauncher.com");
        assert_eq!(site!("/wrath"), "https://leylinelauncher.com/wrath");
        assert_eq!(relay!("/api/listing/realms.json"), "https://relay.leylinelauncher.com/api/listing/realms.json");
        assert!(site!("").ends_with(SITE_HOST) && relay!("").ends_with(RELAY_HOST));
    }

    #[test]
    fn fallback_keeps_path_and_query() {
        assert_eq!(fallback("https://leylinelauncher.com/wrath/mods/all.json?x=1").as_deref(), Some("https://play.openvanilla.app/wrath/mods/all.json?x=1"));
        assert_eq!(fallback("https://play.leylinelauncher.com/").as_deref(), Some("https://wrath.openvanilla.app/"));
        assert_eq!(fallback("wss://relay.leylinelauncher.com/agent").as_deref(), Some("wss://relay.openvanilla.app/agent"));
        assert_eq!(fallback("https://LEYLINELAUNCHER.COM").as_deref(), Some("https://play.openvanilla.app"));
        assert_eq!(fallback("https://play.openvanilla.app/wrath"), None);
        assert_eq!(fallback("https://github.com/orionopenwow/Leyline"), None);
        assert_eq!(fallback("https://leylinelauncher.com.evil.example/x"), None);
        assert_eq!(fallback("not a url"), None);
    }

    #[test]
    fn same_site_and_realms() {
        assert!(same_site("https://play.openvanilla.app", "https://leylinelauncher.com"));
        assert!(same_site("http://127.0.0.1:9", "http://127.0.0.1:9"));
        assert!(!same_site("https://evil.example", "https://leylinelauncher.com"));
        assert!(!same_site("https://wrath.openvanilla.app", "https://leylinelauncher.com"));
        assert!(is_official_realm("play.openvanilla.app") && is_official_realm("leylinelauncher.com:3724"));
        assert!(!is_official_realm("relay.leylinelauncher.com:40002"));
        assert_eq!(current("relay.openvanilla.app"), RELAY_HOST);
        assert_eq!(current("v1.example.org"), "v1.example.org");
        assert!(is_official_realm(official_realm()));
        assert_eq!(site_path("https://play.openvanilla.app/wrath/x", SITE), Some("/wrath/x"));
        assert_eq!(site_path("https://leylinelauncher.com/a", "https://play.openvanilla.app"), Some("/a"));
        assert_eq!(site_path("https://evil.example/a", SITE), None);
        assert_eq!(site_path("https://leylinelauncher.com", SITE), None);
    }

    #[test]
    fn the_server_is_off() {
        assert!(!SERVER_FEATURES, "Leyline is a launcher aggregator: no server of its own");
        for dead in ["https://relay.leylinelauncher.com/api/listing/realms.json", "wss://relay.openvanilla.app/agent",
            "https://play.leylinelauncher.com/", "https://wrath.openvanilla.app/x", "https://play.openvanilla.app/wrath/mods/all.json",
            "https://leylinelauncher.com/wrath/api/social/ways", "https://leylinelauncher.com/wrath/forum/api/threads",
            "https://leylinelauncher.com/wrath/bugs/api/report", "https://leylinelauncher.com/auth/google/start",
            "https://leylinelauncher.com/wrath/status.json", "https://LEYLINELAUNCHER.COM/wrath/api/dens?x=1"] {
            assert!(off(dead), "{dead}");
        }
        for live in ["https://leylinelauncher.com/wrath/mods/all.json", "https://leylinelauncher.com/wrath/leyline/news.json",
            "https://leylinelauncher.com/wrath/realms.json", "https://leylinelauncher.com/wrath/leyline/flags.json",
            "https://leylinelauncher.com/wrath/modes.json", "https://leylinelauncher.com/wrath/docs/hosting",
            "https://api.github.com/repos/orionopenwow/Leyline/releases?per_page=30", "http://127.0.0.1:9/x", "not a url"] {
            assert!(!off(live), "{live}");
        }
        assert!(dead_host("relay.leylinelauncher.com") && dead_host("RELAY.OPENVANILLA.APP") && !dead_host("leylinelauncher.com"));
        // never called: the call is not made, and the old name is not tried for a site file that did not answer
        let mut seen = vec![];
        let r: Result<u8, ureq::Error> = try_both("https://relay.leylinelauncher.com/x", |u| { seen.push(u.to_string()); Ok(1) });
        assert!(matches!(r, Err(ureq::Error::Transport(_))));
        assert!(seen.is_empty());
        let _ = try_both("https://leylinelauncher.com/x", |u| { seen.push(u.to_string()); ureq::get("http://127.0.0.1:1/").call().map(|_| 0u8) });
        assert_eq!(seen, ["https://leylinelauncher.com/x"], "no second try on the old name");
    }

    #[test]
    #[ignore = "the old name is off while SERVER_FEATURES is off"]
    fn try_both_only_on_no_answer() {
        let mut seen = vec![];
        let r: Result<u8, ureq::Error> = try_both("https://leylinelauncher.com/x", |u| {
            seen.push(u.to_string());
            if u.contains("leylinelauncher") {
                ureq::get("http://127.0.0.1:1/").call().map(|_| 0)
            } else {
                Ok(7)
            }
        });
        assert_eq!(r.unwrap(), 7);
        assert_eq!(seen, ["https://leylinelauncher.com/x", "https://play.openvanilla.app/x"]);
    }

    #[test]
    fn no_old_name_retry_once_the_new_name_answered() {
        let down = || ureq::get("http://127.0.0.1:1/").call().map(|_| 0u8);
        let mut seen = vec![];
        // the new name answers once (any HTTP answer counts)
        let r: Result<u8, ureq::Error> = try_both("https://leylinelauncher.com/a", |u| { seen.push(u.to_string()); Ok(1) });
        assert_eq!(r.unwrap(), 1);
        // then the server is unreachable: one try, no second wait on the old name
        let r: Result<u8, ureq::Error> = try_both("https://leylinelauncher.com/b", |u| { seen.push(u.to_string()); down() });
        assert!(matches!(r, Err(ureq::Error::Transport(_))));
        assert_eq!(seen, ["https://leylinelauncher.com/a", "https://leylinelauncher.com/b"]);
    }

    #[test]
    #[ignore = "the old name is off while SERVER_FEATURES is off"]
    fn a_local_address_never_marks_the_new_name() {
        let r: Result<u8, ureq::Error> = try_both("http://127.0.0.1:9/x", |_| Ok(1));
        assert_eq!(r.unwrap(), 1);
        let mut seen = vec![];
        let _ = try_both("https://leylinelauncher.com/x", |u| { seen.push(u.to_string()); ureq::get("http://127.0.0.1:1/").call().map(|_| 0u8) });
        assert_eq!(seen.len(), 2, "still tries the old name: {seen:?}");
    }
}
