//! The launcher's name, look and network addresses, from `launcher/branding.json` (built in: `include_str!`).
//!
//! One file a community changes to run the whole thing under another name with its own keys (docs/FORKING.md):
//! the name, colours and logo, the update sources (GitHub releases and mirrors), the release log's public key and
//! where to fetch it, the relays and the flags file. `launcher/scripts/rebrand.py` copies a fork's values into
//! the few places that need them as text (tauri.conf.json, the UI, the `const`s other modules use); the test
//! `the_code_says_what_branding_json_says` fails when they drift apart.
//!
//! Nothing here is a secret: public keys and public addresses only.

use serde::Deserialize;
use std::sync::OnceLock;

pub const RAW: &str = include_str!("../../branding.json");

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Colours {
    pub bg: String,
    pub text: String,
    pub accent: String,
    pub accent_strong: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Updates {
    pub tag_prefix: String,
    pub github_api: String,
    pub github_downloads: String,
    pub releases_page: String,
    /// Our own copies of every release (`<mirror>latest.json`, `<mirror>launcher-v<version>/<file>`), tried after
    /// GitHub. Each download is still checked against the updater's public key.
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// Spare updater public keys (base64 minisign, like tauri.conf.json's): an update signed with one of these
    /// installs too, so a lost main signing key does not strand every installed launcher. The owner makes them
    /// offline (docs/RESILIENCE.md).
    #[serde(default)]
    pub backup_pubkeys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ReleaseLog {
    /// Hex Ed25519 public key; empty until the owner makes the release key (the log is then not read at all).
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Branding {
    pub format: u64,
    pub name: String,
    pub identifier: String,
    pub colours: Colours,
    pub logo: String,
    pub emblem: String,
    pub site: String,
    pub repo: String,
    pub updates: Updates,
    pub release_log: ReleaseLog,
    pub relays: Vec<String>,
    pub flags_url: String,
}

fn colour_ok(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// Check a branding document: every address plain https, colours `#rrggbb`, the key empty or 64 hex digits.
pub fn parse(text: &str) -> Result<Branding, String> {
    let b: Branding = serde_json::from_str(text).map_err(|e| format!("branding.json: {e}"))?;
    if b.format != 1 {
        return Err("branding.json: unknown format".into());
    }
    if b.name.trim().is_empty() || b.name.len() > 40 {
        return Err("branding.json: name must be 1-40 characters".into());
    }
    if !b.repo.split_once('/').is_some_and(|(o, r)| !o.is_empty() && !r.is_empty() && !r.contains('/')) {
        return Err("branding.json: repo must be owner/name".into());
    }
    for c in [&b.colours.bg, &b.colours.text, &b.colours.accent, &b.colours.accent_strong] {
        if !colour_ok(c) {
            return Err(format!("branding.json: colour {c:?} is not #rrggbb"));
        }
    }
    let mut urls: Vec<&String> = vec![&b.site, &b.updates.github_api, &b.updates.github_downloads, &b.updates.releases_page, &b.flags_url];
    urls.extend(b.updates.mirrors.iter());
    urls.extend(b.release_log.urls.iter());
    urls.extend(b.relays.iter());
    for u in urls {
        if !crate::net::plain_https(u) {
            return Err(format!("branding.json: {u:?} is not a plain https address"));
        }
    }
    for m in b.updates.mirrors.iter().chain([&b.updates.github_downloads]) {
        if !m.ends_with('/') {
            return Err(format!("branding.json: {m:?} must end with /"));
        }
    }
    let k = &b.release_log.public_key;
    if !(k.is_empty() || (k.len() == 64 && k.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)))) {
        return Err("branding.json: release_log.public_key must be empty or 64 lower-case hex digits".into());
    }
    if let Some(k) = b.updates.backup_pubkeys.iter().find(|k| !crate::update::pubkey_ok(k)) {
        return Err(format!("branding.json: updates.backup_pubkeys: {k:?} is not a minisign public key"));
    }
    if b.updates.tag_prefix.is_empty() || b.updates.tag_prefix.contains('/') {
        return Err("branding.json: updates.tag_prefix".into());
    }
    Ok(b)
}

/// The built-in branding. The file is checked by the tests, so this never fails in a tested build.
pub fn get() -> &'static Branding {
    static B: OnceLock<Branding> = OnceLock::new();
    B.get_or_init(|| parse(RAW).expect("launcher/branding.json is checked by branding::tests"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_branding_is_valid() {
        let b = get();
        assert!(!b.name.trim().is_empty());
        assert!(b.updates.mirrors.iter().all(|m| m.starts_with("https://")));
        assert!(!b.release_log.urls.is_empty());
        // The logo and emblem are files in launcher/.
        let launcher = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(launcher.join(&b.logo).is_file(), "{}", b.logo);
        assert!(launcher.join(&b.emblem).is_file(), "{}", b.emblem);
    }

    #[test]
    fn the_code_says_what_branding_json_says() {
        // The constants other modules use, and tauri.conf.json, must match the one file (rebrand.py keeps them so).
        let b = get();
        assert_eq!(crate::update::REPO, b.repo);
        assert_eq!(crate::update::URL, b.updates.github_api);
        assert_eq!(crate::update::DOWNLOAD_PREFIX, b.updates.github_downloads);
        assert_eq!(crate::update::RELEASES_PAGE, b.updates.releases_page);
        assert_eq!(crate::update::TAG_PREFIX, b.updates.tag_prefix);
        assert_eq!(crate::p2p::flags::URL, b.flags_url);
        assert_eq!(crate::p2p::mesh::SEEDS.iter().map(|s| s.to_string()).collect::<Vec<_>>(), b.relays);
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["productName"], b.name.as_str());
        assert_eq!(conf["identifier"], b.identifier.as_str());
    }

    #[test]
    fn bad_branding_is_refused() {
        let good: serde_json::Value = serde_json::from_str(RAW).unwrap();
        let with = |f: &dyn Fn(&mut serde_json::Value)| {
            let mut v = good.clone();
            f(&mut v);
            parse(&v.to_string())
        };
        assert!(with(&|_| {}).is_ok());
        assert!(with(&|v| v["updates"]["mirrors"] = serde_json::json!(["http://plain.example.com/"])).is_err());
        assert!(with(&|v| v["updates"]["mirrors"] = serde_json::json!(["https://m.example.com/no-slash"])).is_err());
        assert!(with(&|v| v["release_log"]["public_key"] = "xyz".into()).is_err());
        assert!(with(&|v| v["release_log"]["public_key"] = "ab".repeat(32).into()).is_ok());
        assert!(with(&|v| v["colours"]["accent"] = "blue".into()).is_err());
        assert!(with(&|v| v["repo"] = "noslash".into()).is_err());
        assert!(with(&|v| v["format"] = 2.into()).is_err());
        assert!(with(&|v| v["updates"]["backup_pubkeys"] = serde_json::json!(["TODO"])).is_err());
        let pk = serde_json::from_str::<serde_json::Value>(include_str!("../tauri.conf.json")).unwrap()["plugins"]["updater"]["pubkey"].clone();
        if crate::update::pubkey_ok(pk.as_str().unwrap()) {
            assert!(with(&|v| v["updates"]["backup_pubkeys"] = serde_json::json!([pk])).is_ok());
        }
    }
}
