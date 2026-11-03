//! Leyline's public addresses: the one place in the launcher (docs/DOMAIN-MOVE.md; the same list is
//! `tools/domain/addresses.json`, which `tools/domain/tests/test_domain.py` checks this file against).
//!
//! Every web address the launcher calls or opens is built from these with `site!`, `relay!`. The new names come first;
//! the old ones keep answering forever, so a request that cannot reach a new name (no DNS yet, a network that blocks
//! it) is tried once more on the old one (`fallback`, used by `net`, `social`, `bugs` and `rankings`). Downloads
//! are accepted from the site on either name (`same_site`). The release check (GitHub) and deep links do not change.

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

/// Run `call` on `url`; when it could not reach the server at all (DNS, connect, TLS: not an HTTP answer) and
/// `url` is on a new name, run it once more on the old name.
pub fn try_both<T>(url: &str, mut call: impl FnMut(&str) -> Result<T, ureq::Error>) -> Result<T, ureq::Error> {
    match call(url) {
        Err(ureq::Error::Transport(t)) => match fallback(url) {
            Some(old) => call(&old),
            None => Err(ureq::Error::Transport(t)),
        },
        other => other,
    }
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
}
