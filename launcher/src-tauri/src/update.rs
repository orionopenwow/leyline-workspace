//! Leyline's own updates: the GitHub releases of the PUBLIC repo orionopenwow/Leyline, tags `launcher-v*`.
//!
//! Two parts:
//! * The check (plain Rust, tested here): the newest release of the player's channel (stable: releases only;
//!   beta: pre-releases too), cached for offline use, and that release's updater manifest address
//!   (`.../releases/download/launcher-v<version>/latest.json`, made by CI).
//! * The install (the desktop app, `app.rs`): Tauri's updater plugin fetches that one manifest, checks the
//!   download's signature against the public key in tauri.conf.json, downloads it in the background and installs
//!   it when the player presses "Restart" in the title bar, never while a game Leyline started is running. Left
//!   waiting, it is installed at the next start (`READY_FILE`, `finish_now`).
//!   Until the owner has run "setup\Turn on launcher updates.bat" (no public key yet), or on a .deb install, Leyline
//!   only links to the release page.
//!
//! `parse_manifest` re-checks the manifest Leyline is about to hand the plugin: a semver version, a signature
//! per platform, and download addresses only under this repo's release downloads.

use crate::net;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

pub const REPO: &str = "orionopenwow/Leyline";
pub const URL: &str = "https://api.github.com/repos/orionopenwow/Leyline/releases?per_page=30";
pub const RELEASES_PAGE: &str = "https://github.com/orionopenwow/Leyline/releases";
pub const DOWNLOAD_PREFIX: &str = "https://github.com/orionopenwow/Leyline/releases/download/";
pub const TAG_PREFIX: &str = "launcher-v";
pub const MANIFEST_NAME: &str = "latest.json";
/// How often the app looks for a new version while it runs (and once at start).
pub const CHECK_EVERY_SECS: u64 = 6 * 60 * 60;

/// Which releases the player wants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Releases only.
    #[default]
    Stable,
    /// Pre-releases too.
    Beta,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub channel: Channel,
    /// The newest launcher release of the channel, when one was found.
    pub latest: Option<String>,
    pub newer: bool,
    /// Its release page (only ever a page under RELEASES_PAGE).
    pub url: Option<String>,
    /// Its updater manifest (only ever under DOWNLOAD_PREFIX), when the release lists one.
    pub manifest_url: Option<String>,
    pub prerelease: bool,
    pub from_cache: bool,
    pub error: Option<String>,
}

/// One release of the channel: (version, page, manifest address if attached, prerelease).
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    pub manifest_url: Option<String>,
    pub prerelease: bool,
}

/// A semver-ish version: (major, minor, patch) and the pre-release part (`beta.1`), empty for a release.
pub fn parse_version(v: &str) -> Option<((u64, u64, u64), String)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, p.split('+').next().unwrap_or("").to_string()),
        None => (v.split('+').next().unwrap_or(v), String::new()),
    };
    let mut it = core.split('.');
    let n = |x: Option<&str>| -> Option<u64> { x.map_or(Some(0), |s| s.parse().ok()) };
    let a = it.next()?.parse().ok()?;
    let t = (a, n(it.next())?, n(it.next())?);
    if it.next().is_some() || pre.len() > 40 || !pre.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
        return None;
    }
    Some((t, pre))
}

/// Semver order: numbers first, then a release above its pre-releases, pre-release parts compared numerically
/// when both are numbers.
pub fn cmp_versions(a: &str, b: &str) -> Option<Ordering> {
    let (ta, pa) = parse_version(a)?;
    let (tb, pb) = parse_version(b)?;
    Some(ta.cmp(&tb).then_with(|| match (pa.is_empty(), pb.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => {
            let (xa, xb): (Vec<&str>, Vec<&str>) = (pa.split('.').collect(), pb.split('.').collect());
            for (x, y) in xa.iter().zip(&xb) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(i), Ok(j)) => i.cmp(&j),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    _ => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
            xa.len().cmp(&xb.len())
        }
    }))
}

/// `latest` is newer than `have`.
pub fn is_newer(latest: &str, have: &str) -> bool {
    cmp_versions(latest, have) == Some(Ordering::Greater)
}

/// The newest `launcher-v*` release of the channel in a GitHub releases list.
pub fn latest(v: &Value, channel: Channel) -> Option<Release> {
    v.as_array()?
        .iter()
        .filter(|r| !r.get("draft").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?;
            let ver = tag.strip_prefix(TAG_PREFIX)?;
            parse_version(ver)?;
            let prerelease = r.get("prerelease").and_then(Value::as_bool).unwrap_or(false);
            if prerelease && channel == Channel::Stable {
                return None;
            }
            let page = r.get("html_url").and_then(Value::as_str).filter(|u| u.starts_with(&format!("{RELEASES_PAGE}/")) && net::plain_https(u))?;
            let want = format!("{DOWNLOAD_PREFIX}{tag}/{MANIFEST_NAME}");
            let manifest_url = r
                .get("assets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|a| a.get("browser_download_url").and_then(Value::as_str))
                .find(|u| *u == want)
                .map(String::from);
            Some(Release { version: ver.chars().take(48).collect(), page: page.to_string(), manifest_url, prerelease })
        })
        .max_by(|a, b| cmp_versions(&a.version, &b.version).unwrap_or(Ordering::Equal))
}

pub fn check(app_dir: &Path, current: &str, channel: Channel) -> UpdateInfo {
    check_from(URL, app_dir, current, channel)
}

pub fn check_from(url: &str, app_dir: &Path, current: &str, channel: Channel) -> UpdateInfo {
    let cache = app_dir.join("cache").join("releases.json");
    let mut out = UpdateInfo { current: current.into(), channel, ..Default::default() };
    match net::fetch_json(url, &cache, net::TIMEOUT, |v| v.as_array().map(|_| ()).ok_or_else(|| "not a release list".into())) {
        Ok(f) => {
            out.from_cache = f.from_cache;
            out.error = f.error;
            if let Some(r) = latest(&f.value, channel) {
                out.newer = is_newer(&r.version, current);
                out.latest = Some(r.version);
                out.url = Some(r.page);
                out.manifest_url = r.manifest_url;
                out.prerelease = r.prerelease;
            }
        }
        Err(e) => out.error = Some(e),
    }
    out
}

// ------------------------------------------------------------------------------------------------ finishing on the next start

/// When an update is downloaded and the player does not press Restart, Leyline notes its version here; the next
/// start downloads it again (the signature is checked again) and installs it straight away, before anything else
/// (owner 31 Oct: "auto queued for restart update for everyone").
pub const READY_FILE: &str = "update-ready.txt";

pub fn read_ready(app_dir: &Path) -> Option<String> {
    let t = std::fs::read_to_string(app_dir.join(READY_FILE)).ok()?;
    let v = t.trim().to_string();
    parse_version(&v).map(|_| v)
}

pub fn write_ready(app_dir: &Path, version: &str) {
    if parse_version(version).is_some() {
        let _ = crate::settings::write_atomic(&app_dir.join(READY_FILE), version.as_bytes());
    }
}

pub fn clear_ready(app_dir: &Path) {
    let _ = std::fs::remove_file(app_dir.join(READY_FILE));
}

/// The note from the last run, if it still means something: a version newer than this build. An older or equal
/// one (the update went in) is cleared.
pub fn take_ready_at_start(app_dir: &Path, current: &str) -> Option<String> {
    let v = read_ready(app_dir);
    match v {
        Some(v) if is_newer(&v, current) => Some(v),
        _ => {
            clear_ready(app_dir);
            None
        }
    }
}

/// Install the just-downloaded `downloaded` right away, with no Restart press: only on the first check after a
/// start that found the last run's note (`waiting`), when it is at least that version and newer than this build,
/// and never while a game Leyline started is running.
pub fn finish_now(waiting: Option<&str>, current: &str, downloaded: &str, game_running: bool) -> bool {
    let Some(w) = waiting else { return false };
    !game_running
        && is_newer(downloaded, current)
        && matches!(cmp_versions(downloaded, w), Some(Ordering::Greater | Ordering::Equal))
}

// ------------------------------------------------------------------------------------------------ the manifest

/// One platform's download in latest.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub signature: String,
    pub url: String,
}

/// The updater manifest (Tauri's static JSON format) as CI writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: String,
    pub platforms: BTreeMap<String, PlatformAsset>,
}

/// The platform keys Leyline ships: Windows, macOS (one universal app, listed for both CPUs) and Linux (AppImage).
pub const PLATFORM_KEYS: &[&str] = &["windows-x86_64", "darwin-x86_64", "darwin-aarch64", "linux-x86_64"];

/// This build's key in `platforms` (`<os>-<arch>`, as the updater plugin looks it up).
pub fn platform_key(os: &str, arch: &str) -> String {
    let os = match os {
        "macos" => "darwin",
        o => o,
    };
    format!("{os}-{arch}")
}

pub fn this_platform() -> String {
    platform_key(std::env::consts::OS, std::env::consts::ARCH)
}

/// A minisign signature as the updater writes it: base64 text (of a two-line minisign file).
fn signature_ok(s: &str) -> bool {
    (80..=2000).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
}

/// The updater's public key as tauri.conf.json carries it (base64 of a minisign public key file). The repo
/// ships a placeholder until the owner runs "setup\Turn on launcher updates.bat".
pub fn pubkey_ok(s: &str) -> bool {
    signature_ok(s.trim()) && s.trim().len() < 400
}

/// Parse and check latest.json. Platforms with a download outside this repo's releases, or without a
/// signature, are refused (the whole manifest, so a half-bad one is never used).
pub fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let m = parse_manifest_from(text, DOWNLOAD_PREFIX)?;
    if let Some((k, a)) = m.platforms.iter().find(|(_, a)| !net::plain_https(&a.url)) {
        return Err(format!("{k}: the download is not in this release ({})", a.url));
    }
    Ok(m)
}

/// The same checks for a manifest from any update source: every download under
/// `<download_prefix><tag prefix><version>/` of that source (GitHub's releases, our mirror, a mirror the signed
/// release log names). The prefix comes from this build (branding.json) or the release log, never from the reply.
pub fn parse_manifest_from(text: &str, download_prefix: &str) -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(text).map_err(|e| format!("not an update manifest: {e}"))?;
    if parse_version(&m.version).is_none() {
        return Err(format!("the manifest's version {:?} is not a version", m.version));
    }
    if m.platforms.is_empty() {
        return Err("the manifest lists no downloads".into());
    }
    for (k, a) in &m.platforms {
        if !k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_')) {
            return Err(format!("odd platform name {k:?}"));
        }
        let tag = format!("{download_prefix}{TAG_PREFIX}{}/", m.version.trim_start_matches('v'));
        if !a.url.starts_with(&tag) || a.url.contains("..") || a.url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!("{k}: the download is not in this release ({})", a.url));
        }
        if !signature_ok(&a.signature) {
            return Err(format!("{k}: no usable signature"));
        }
    }
    Ok(m)
}

/// The manifest's download for this platform, if it has one.
pub fn asset_for<'a>(m: &'a Manifest, key: &str) -> Option<&'a PlatformAsset> {
    m.platforms.get(key)
}

// ------------------------------------------------------------------------------------------------ several sources

/// One place an update can come from. Whatever it is, the download is installed only when its signature checks
/// against the updater's public key (tauri.conf.json): a source can be down or wrong, it can't push a bad build.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Source {
    /// Plain words for the Settings page: "GitHub", "our server", "mirror m.example.org".
    pub name: String,
    pub manifest_url: String,
    /// Downloads in its manifest must be under `<download_prefix><tag prefix><version>/`.
    pub download_prefix: String,
    /// The manifest's sha256, when the signed release log names it.
    pub manifest_sha256: Option<String>,
    /// The version the manifest must say (GitHub: its release's tag; the release log: its entry).
    pub expect_version: Option<String>,
}

fn host_of(u: &str) -> String {
    net::origin(u).map(|o| o.split("://").nth(1).unwrap_or("").to_string()).unwrap_or_default()
}

/// The sources to try, in order: GitHub's release (when the release check found one), then our own mirrors
/// (branding.json), then the mirrors the signed release log names for its newest launcher above `current`.
pub fn sources(info: &UpdateInfo, mirrors: &[String], log: Option<&crate::releaselog::Chain>) -> Vec<Source> {
    let mut out = Vec::new();
    if let (true, Some(url)) = (info.newer, info.manifest_url.as_ref()) {
        out.push(Source { name: "GitHub".into(), manifest_url: url.clone(), download_prefix: DOWNLOAD_PREFIX.into(), manifest_sha256: None, expect_version: info.latest.clone() });
    }
    for m in mirrors.iter().filter(|m| net::plain_https(m) && m.ends_with('/')) {
        out.push(Source { name: format!("mirror {}", host_of(m)), manifest_url: format!("{m}{MANIFEST_NAME}"), download_prefix: m.clone(), manifest_sha256: None, expect_version: None });
    }
    if let Some(e) = log.and_then(|c| c.newest_launcher_above(&info.current)) {
        let sha = e.files.iter().find(|f| f.name == MANIFEST_NAME).map(|f| f.sha256.clone());
        for m in &e.mirrors {
            let url = format!("{m}{TAG_PREFIX}{}/{MANIFEST_NAME}", e.version);
            if out.iter().any(|s| s.manifest_url == url) {
                continue;
            }
            out.push(Source { name: format!("release log, {}", host_of(m)), manifest_url: url, download_prefix: m.clone(), manifest_sha256: sha.clone(), expect_version: Some(e.version.clone()) });
        }
    }
    out
}

/// A source whose manifest passed every check and offers a version above the running one.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub source: Source,
    pub manifest: Manifest,
}

/// Fetch and check each source's manifest. Kept in source order: a source that is down, answers junk, names
/// another version than it should, points its downloads elsewhere, or offers a version that is not above
/// `current` (never a downgrade) is left out with a note.
pub fn candidates(sources: &[Source], current: &str, platform: &str, fetch: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> (Vec<Candidate>, Vec<String>) {
    use sha2::{Digest, Sha256};
    let (mut good, mut notes) = (Vec::new(), Vec::new());
    for s in sources {
        let r = (|| {
            let body = fetch(&s.manifest_url)?;
            if body.len() > 256 * 1024 {
                return Err("the manifest is too big".to_string());
            }
            if let Some(want) = &s.manifest_sha256 {
                if &net::hex(&Sha256::digest(&body)) != want {
                    return Err("the manifest is not the one the release log names".into());
                }
            }
            let m = parse_manifest_from(&String::from_utf8_lossy(&body), &s.download_prefix)?;
            if let Some(v) = &s.expect_version {
                if cmp_versions(&m.version, v) != Some(Ordering::Equal) {
                    return Err(format!("the manifest says {} but this source is for {v}", m.version));
                }
            }
            if !is_newer(&m.version, current) {
                return Err(format!("{} is not newer than this Leyline ({current}): never installed", m.version));
            }
            if asset_for(&m, platform).is_none() {
                return Err(format!("no download for {platform}"));
            }
            Ok(m)
        })();
        match r {
            Ok(m) => good.push(Candidate { source: s.clone(), manifest: m }),
            Err(e) => notes.push(format!("{}: {e}", s.name)),
        }
    }
    (good, notes)
}

/// Try candidates in order until one works (`try_one`: download and check the signature). The first that works
/// wins; every failure is noted.
pub fn first_good<T>(cands: &[Candidate], mut try_one: impl FnMut(&Candidate) -> Result<T, String>) -> Result<(usize, T), Vec<String>> {
    let mut notes = Vec::new();
    for (i, c) in cands.iter().enumerate() {
        match try_one(c) {
            Ok(t) => return Ok((i, t)),
            Err(e) => notes.push(format!("{}: {e}", c.source.name)),
        }
    }
    Err(notes)
}

/// Standard base64 (with or without padding) to bytes.
pub fn b64decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.trim().bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The updater's own check, here in Rust so it can be tested with fake sources and used for files that do not go
/// through the updater plugin: `data` signed (minisign, as `tauri signer` makes it) by the key whose base64 public
/// key file is `pubkey_b64`; `sig_b64` as latest.json carries it.
pub fn verify_minisign(pubkey_b64: &str, data: &[u8], sig_b64: &str) -> Result<(), String> {
    let pk_text = b64decode(pubkey_b64).and_then(|b| String::from_utf8(b).ok()).ok_or("the public key is not base64")?;
    let sig_text = b64decode(sig_b64).and_then(|b| String::from_utf8(b).ok()).ok_or("the signature is not base64")?;
    let pk = minisign_verify::PublicKey::decode(&pk_text).map_err(|e| format!("the public key is not a minisign key ({e})"))?;
    let sig = minisign_verify::Signature::decode(&sig_text).map_err(|e| format!("not a minisign signature ({e})"))?;
    pk.verify(data, &sig, false).map_err(|_| "the signature does not match: not installed".to_string())
}

/// The public key built into this app (tauri.conf.json `plugins.updater.pubkey`).
pub fn built_in_pubkey() -> String {
    let conf: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap_or(Value::Null);
    conf["plugins"]["updater"]["pubkey"].as_str().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::testserver::*;

    const LIST: &str = r#"[
      {"tag_name":"launcher-v0.4.0","draft":true,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.4.0"},
      {"tag_name":"launcher-v0.3.1-beta.2","draft":false,"prerelease":true,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.3.1-beta.2",
       "assets":[{"browser_download_url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.1-beta.2/latest.json"}]},
      {"tag_name":"launcher-v0.3.0","draft":false,"prerelease":false,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.3.0",
       "assets":[{"browser_download_url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/Leyline_0.3.0_x64-setup.exe"},
                 {"browser_download_url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/latest.json"}]},
      {"tag_name":"v9.0.0","draft":false,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/v9.0.0"},
      {"tag_name":"launcher-v0.2.0","draft":false,"prerelease":true,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.2.0"},
      {"tag_name":"launcher-v5.0.0","draft":false,"html_url":"https://evil.example.com/x"}
    ]"#;

    #[test]
    fn finds_the_newest_release_of_each_channel() {
        let v: Value = serde_json::from_str(LIST).unwrap();
        let s = latest(&v, Channel::Stable).unwrap();
        assert_eq!(s.version, "0.3.0", "drafts, pre-releases, other tags and other sites are left out");
        assert!(s.page.ends_with("/tag/launcher-v0.3.0") && !s.prerelease);
        assert_eq!(s.manifest_url.as_deref(), Some("https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/latest.json"));
        let b = latest(&v, Channel::Beta).unwrap();
        assert_eq!(b.version, "0.3.1-beta.2");
        assert!(b.prerelease && b.manifest_url.unwrap().ends_with("/launcher-v0.3.1-beta.2/latest.json"));
    }

    #[test]
    fn versions_in_semver_order() {
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(is_newer("0.3.0", "0.3.0-beta.9"));
        assert!(is_newer("0.3.0-beta.10", "0.3.0-beta.9"));
        assert!(is_newer("0.3.0-rc.1", "0.3.0-beta.1"));
        assert!(!is_newer("0.3.0-beta.1", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("x", "0.1.0"));
        assert_eq!(parse_version("v1.2"), Some(((1, 2, 0), String::new())));
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("1.2.3-be ta"), None);
    }

    #[test]
    fn says_newer_or_not_per_channel_and_works_offline() {
        let d = tempfile::tempdir().unwrap();
        let base = serve_routes(vec![("/releases", "200 OK", LIST.as_bytes().to_vec())]);
        let u = check_from(&format!("{base}/releases"), d.path(), "0.2.0", Channel::Stable);
        assert!(u.newer && u.latest.as_deref() == Some("0.3.0") && !u.from_cache && u.manifest_url.is_some());
        assert!(!check_from(&format!("{base}/releases"), d.path(), "0.3.0", Channel::Stable).newer);
        let b = check_from(&format!("{base}/releases"), d.path(), "0.3.0", Channel::Beta);
        assert!(b.newer && b.prerelease && b.channel == Channel::Beta);
        let off = check_from(&dead_url(), d.path(), "0.2.0", Channel::Stable);
        assert!(off.newer && off.from_cache);
        let none = check_from(&dead_url(), tempfile::tempdir().unwrap().path(), "0.1.0", Channel::Stable);
        assert!(!none.newer && none.error.is_some());
    }

    #[test]
    fn a_downloaded_update_finishes_on_the_next_start() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_ready(d.path()), None);
        write_ready(d.path(), "0.3.4");
        assert_eq!(read_ready(d.path()).as_deref(), Some("0.3.4"));
        write_ready(d.path(), "not a version");
        assert_eq!(read_ready(d.path()).as_deref(), Some("0.3.4"), "only versions are written");
        // the next start, still on the old build: the note stands
        assert_eq!(take_ready_at_start(d.path(), "0.3.2").as_deref(), Some("0.3.4"));
        assert!(d.path().join(READY_FILE).exists());
        // the start after the update went in: the note is cleared
        assert_eq!(take_ready_at_start(d.path(), "0.3.4"), None);
        assert!(!d.path().join(READY_FILE).exists());
        write_ready(d.path(), "0.3.4");
        assert_eq!(take_ready_at_start(d.path(), "0.4.0"), None);
        std::fs::write(d.path().join(READY_FILE), "garbage").unwrap();
        assert_eq!(take_ready_at_start(d.path(), "0.3.0"), None);

        // installs by itself only with the note, a newer download, and no game open
        assert!(finish_now(Some("0.3.4"), "0.3.2", "0.3.4", false));
        assert!(finish_now(Some("0.3.4"), "0.3.2", "0.3.5", false), "a newer one than the note");
        assert!(!finish_now(Some("0.3.4"), "0.3.2", "0.3.4", true), "never while a game runs");
        assert!(!finish_now(None, "0.3.2", "0.3.4", false), "a fresh download waits for Restart");
        assert!(!finish_now(Some("0.3.4"), "0.3.2", "0.3.3", false), "an older one than the note");
        assert!(!finish_now(Some("0.3.4"), "0.3.4", "0.3.4", false), "nothing newer than this build");
    }

    #[test]
    fn every_ship_is_a_stable_release() {
        // CI's ship releases are plain versions (launcher/scripts/release_version.py), so the default channel,
        // stable, gets each one; the beta channel sees them too, plus pre-releases.
        let v: Value = serde_json::from_str(r#"[
          {"tag_name":"launcher-v0.3.2","prerelease":false,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.3.2"},
          {"tag_name":"launcher-v0.3.1","prerelease":false,"html_url":"https://github.com/orionopenwow/Leyline/releases/tag/launcher-v0.3.1"}]"#).unwrap();
        assert_eq!(Channel::default(), Channel::Stable);
        assert_eq!(latest(&v, Channel::Stable).unwrap().version, "0.3.2");
        assert_eq!(latest(&v, Channel::Beta).unwrap().version, "0.3.2");
    }

    fn sig() -> String {
        "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSVEVTVA".repeat(3)
    }

    fn manifest(url_win: &str) -> String {
        let s = sig();
        format!(
            r#"{{"version":"0.3.0","notes":"Leyline 0.3.0","pub_date":"2026-10-30T12:00:00Z","platforms":{{
            "windows-x86_64":{{"signature":"{s}","url":"{url_win}"}},
            "darwin-x86_64":{{"signature":"{s}","url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/Leyline_universal.app.tar.gz"}},
            "darwin-aarch64":{{"signature":"{s}","url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/Leyline_universal.app.tar.gz"}},
            "linux-x86_64":{{"signature":"{s}","url":"https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/Leyline_0.3.0_amd64.AppImage"}}}}}}"#
        )
    }

    #[test]
    fn parses_and_checks_the_updater_manifest() {
        let good = manifest("https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/Leyline_0.3.0_x64-setup.exe");
        let m = parse_manifest(&good).unwrap();
        assert_eq!(m.version, "0.3.0");
        for k in PLATFORM_KEYS {
            assert!(asset_for(&m, k).is_some(), "{k}");
        }
        assert!(asset_for(&m, "linux-aarch64").is_none());
        // Downloads only from this release of this repo.
        for bad in [
            "https://evil.example.com/Leyline.exe",
            "https://github.com/someone/Leyline/releases/download/launcher-v0.3.0/x.exe",
            "https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.2.0/old.exe",
            "http://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/x.exe",
            "https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.0/../launcher-v0.1.0/x.exe",
        ] {
            assert!(parse_manifest(&manifest(bad)).unwrap_err().contains("not in this release"), "{bad}");
        }
        assert!(parse_manifest(&good.replace(&sig(), "short")).unwrap_err().contains("signature"));
        assert!(parse_manifest(&good.replace("\"0.3.0\"", "\"three\"")).is_err());
        assert!(parse_manifest(r#"{"version":"0.3.0","platforms":{}}"#).unwrap_err().contains("no downloads"));
        assert!(parse_manifest("<html>").is_err());
    }

    // A minisign key and signatures made for these tests only (hosting-side generator: python cryptography +
    // hashlib.blake2b, the minisign "ED" prehashed format the Tauri signer writes). Not the release key.
    const TEST_PUB: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDA4MDcwNjA1MDQwMzAyMDEKUldRQkFnTUVCUVlIQ09wS2JHUGluRklLdnZWUWV4TXV4Zm1WUjNhdXZyNTdra0llNm1rVVJ0SXMK";
    const TEST_SIG_GOOD: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRQkFnTUVCUVlIQ0YwWnFtcjkraVNlbkpua0ZNbjBEaFplSk0vcVJXMmVJWHF5K0xIRlhTbjdWOTNjQWR1QXhldTJBT05tUHBIK3VnN3pXQUJKMlNLTVF6ajl6M2czcXdrPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzYxOTAwMDAwCWZpbGU6TGV5bGluZV8wLjkuMF94NjQtc2V0dXAuZXhlCjZOeTVKOXdxRFh0THBIS3lZWURxYXFzVkNyOGluUFUzajQ1eDF4cXBVREd5MmJwL1pqeW5BdnpOM3F0T2xDMzZvZllQR1lBbDZoMFpId0VQdkNjL0JBPT0K";
    /// The same bundle signed by another key that carries the right key id: a mirror trying to push its own build.
    const TEST_SIG_BAD: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRQkFnTUVCUVlIQ0wweTB1L0FERFh2NDdhUUhRQmRwU1JzZi9KTDlPL1czaFh6d0hobk41a1gyWlMxUVpUc3lKeUhKZ0NvcFgwUGZnM3dnY3RkaUlIWTdvMmJIaitaSVFBPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzYxOTAwMDAwCWZpbGU6TGV5bGluZV8wLjkuMF94NjQtc2V0dXAuZXhlCmh5YmR5MXU1bmprcnFxUlJmblFmU284VStpUEN1MkJmNS9nbk1CTzQvRXRTUWJKeGc2RnJMTDhOdHEyNGxnd1RuVFJocGhMMFRmaGx3cW1iazFocER3PT0K";

    fn bundle() -> Vec<u8> {
        b"LEYLINE TEST BUNDLE 0.9.0\n".repeat(40)
    }

    fn mirror_manifest(prefix: &str, version: &str, sig: &str) -> Vec<u8> {
        json!({"version": version, "notes": "", "pub_date": "2026-10-31T12:00:00Z", "platforms": {
            "windows-x86_64": {"signature": sig, "url": format!("{prefix}launcher-v{version}/Leyline_{version}_x64-setup.exe")},
            "linux-x86_64": {"signature": sig, "url": format!("{prefix}launcher-v{version}/Leyline_{version}_amd64.AppImage")}}})
        .to_string()
        .into_bytes()
    }
    use serde_json::json;

    #[test]
    fn minisign_signatures_are_checked_here_as_the_updater_does() {
        assert!(verify_minisign(TEST_PUB, &bundle(), TEST_SIG_GOOD).is_ok());
        assert!(verify_minisign(TEST_PUB, &bundle(), TEST_SIG_BAD).unwrap_err().contains("does not match"));
        let mut changed = bundle();
        changed[3] ^= 1;
        assert!(verify_minisign(TEST_PUB, &changed, TEST_SIG_GOOD).is_err(), "one changed byte");
        assert!(verify_minisign("not base64!", &bundle(), TEST_SIG_GOOD).is_err());
        // the real built-in key decodes as a minisign key (and does not verify a test signature)
        let real = built_in_pubkey();
        if pubkey_ok(&real) {
            assert!(verify_minisign(&real, &bundle(), TEST_SIG_GOOD).is_err());
            let txt = String::from_utf8(b64decode(&real).unwrap()).unwrap();
            assert!(minisign_verify::PublicKey::decode(&txt).is_ok());
        }
        assert_eq!(b64decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(b64decode("aGVsbG8").unwrap(), b"hello");
    }

    #[test]
    fn several_sources_one_down_one_bad_signature_one_good() {
        // Three update sources: the first is down, the second serves a build whose signature is not ours, the third
        // serves the real one. The update comes from the third; nothing from the second is ever accepted.
        let dead = dead_url().replace("/x.json", "/");
        // Build the servers so each manifest names its own address: bind first, then serve.
        let (bad_base, bad_tx) = lazy_server();
        let (good_base, good_tx) = lazy_server();
        bad_tx.send(vec![
            ("/r/latest.json".to_string(), mirror_manifest(&format!("{bad_base}/r/"), "0.9.0", TEST_SIG_BAD)),
            ("/r/launcher-v0.9.0/Leyline_0.9.0_amd64.AppImage".to_string(), bundle()),
            ("/r/launcher-v0.9.0/Leyline_0.9.0_x64-setup.exe".to_string(), bundle()),
        ]).unwrap();
        good_tx.send(vec![
            ("/r/latest.json".to_string(), mirror_manifest(&format!("{good_base}/r/"), "0.9.0", TEST_SIG_GOOD)),
            ("/r/launcher-v0.9.0/Leyline_0.9.0_amd64.AppImage".to_string(), bundle()),
            ("/r/launcher-v0.9.0/Leyline_0.9.0_x64-setup.exe".to_string(), bundle()),
        ]).unwrap();
        let info = UpdateInfo { current: "0.3.1".into(), ..Default::default() };
        let mirrors = vec![dead, format!("{bad_base}/r/"), format!("{good_base}/r/")];
        let srcs: Vec<Source> = mirrors.iter().map(|m| Source { name: m.clone(), manifest_url: format!("{m}latest.json"), download_prefix: m.clone(), manifest_sha256: None, expect_version: None }).collect();
        // (sources() itself only takes https mirrors; these test servers are plain http on 127.0.0.1)
        assert!(sources(&info, &mirrors, None).is_empty());
        let fetch = |u: &str| -> Result<Vec<u8>, String> {
            let r = ureq::get(u).timeout(std::time::Duration::from_secs(5)).call().map_err(|e| e.to_string())?;
            let mut v = Vec::new();
            std::io::Read::read_to_end(&mut r.into_reader(), &mut v).map_err(|e| e.to_string())?;
            Ok(v)
        };
        let (cands, notes) = candidates(&srcs, "0.3.1", "linux-x86_64", &fetch);
        assert_eq!(cands.len(), 2, "{notes:?}");
        assert_eq!(notes.len(), 1, "the dead one: {notes:?}");
        let (i, bytes) = first_good(&cands, |c| {
            let a = asset_for(&c.manifest, "linux-x86_64").unwrap();
            let data = fetch(&a.url)?;
            verify_minisign(TEST_PUB, &data, &a.signature)?;
            Ok(data)
        })
        .unwrap();
        assert_eq!(cands[i].source.download_prefix, format!("{good_base}/r/"), "the good mirror wins");
        assert_eq!(bytes, bundle());
        // only the bad one: nothing is installed
        let only_bad = vec![cands[0].clone()];
        assert!(first_good(&only_bad, |c| {
            let a = asset_for(&c.manifest, "linux-x86_64").unwrap();
            verify_minisign(TEST_PUB, &fetch(&a.url)?, &a.signature)
        }).unwrap_err()[0].contains("does not match"));
    }

    /// A test server bound now, given its routes later (so a manifest can name the server's own address).
    fn lazy_server() -> (String, std::sync::mpsc::Sender<Vec<(String, Vec<u8>)>>) {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, Vec<u8>)>>();
        std::thread::spawn(move || {
            let routes = rx.recv().unwrap();
            for s in l.incoming() {
                let Ok(mut s) = s else { continue };
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                match routes.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                        let _ = s.write_all(body);
                    }
                    None => {
                        let _ = write!(s, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        (base, tx)
    }

    #[test]
    fn never_a_downgrade_and_never_another_sources_downloads() {
        let p = "https://m.example.org/r/";
        let fetch_with = |body: Vec<u8>| move |_: &str| -> Result<Vec<u8>, String> { Ok(body.clone()) };
        let src = Source { name: "m".into(), manifest_url: format!("{p}latest.json"), download_prefix: p.into(), manifest_sha256: None, expect_version: None };
        // an older or equal version than the running one is never a candidate
        for v in ["0.3.0", "0.3.1", "0.3.1-beta.9"] {
            let (c, n) = candidates(&[src.clone()], "0.3.1", "linux-x86_64", &fetch_with(mirror_manifest(p, v, TEST_SIG_GOOD)));
            assert!(c.is_empty() && n[0].contains("never installed"), "{v}: {n:?}");
        }
        let (c, _) = candidates(&[src.clone()], "0.3.1", "linux-x86_64", &fetch_with(mirror_manifest(p, "0.3.2", TEST_SIG_GOOD)));
        assert_eq!(c.len(), 1);
        // a mirror whose manifest points its downloads somewhere else
        let (c, n) = candidates(&[src.clone()], "0.3.1", "linux-x86_64", &fetch_with(mirror_manifest("https://evil.example.com/r/", "0.3.2", TEST_SIG_GOOD)));
        assert!(c.is_empty() && n[0].contains("not in this release"), "{n:?}");
        // a source for a given version that answers another
        let mut s2 = src.clone();
        s2.expect_version = Some("0.3.3".into());
        let (c, n) = candidates(&[s2], "0.3.1", "linux-x86_64", &fetch_with(mirror_manifest(p, "0.3.2", TEST_SIG_GOOD)));
        assert!(c.is_empty() && n[0].contains("this source is for 0.3.3"), "{n:?}");
        // a manifest the release log names by hash, and one it does not
        let body = mirror_manifest(p, "0.3.2", TEST_SIG_GOOD);
        let mut s3 = src.clone();
        s3.manifest_sha256 = Some(net::hex(&<sha2::Sha256 as sha2::Digest>::digest(&body)));
        assert_eq!(candidates(&[s3.clone()], "0.3.1", "linux-x86_64", &fetch_with(body.clone())).0.len(), 1);
        let (c, n) = candidates(&[s3], "0.3.1", "linux-x86_64", &fetch_with(mirror_manifest(p, "0.3.2", TEST_SIG_BAD)));
        assert!(c.is_empty() && n[0].contains("release log"), "{n:?}");
        // no download for this PC
        let (c, n) = candidates(&[src], "0.3.1", "darwin-aarch64", &fetch_with(mirror_manifest(p, "0.3.2", TEST_SIG_GOOD)));
        assert!(c.is_empty() && n[0].contains("no download"), "{n:?}");
    }

    #[test]
    fn the_sources_in_order() {
        let m = "https://m.example.org/r/";
        let log = crate::releaselog::verify(
            &crate::releaselog::tests::launcher_chain(crate::releaselog::tests::DEV_TEST_SEED, &["0.3.4", "0.3.5"], "https://peer.example.net/leyline/"),
            &crate::releaselog::tests::pub_hex(crate::releaselog::tests::DEV_TEST_SEED),
        )
        .unwrap();
        let info = UpdateInfo { current: "0.3.1".into(), newer: true, latest: Some("0.3.5".into()),
            manifest_url: Some("https://github.com/orionopenwow/Leyline/releases/download/launcher-v0.3.5/latest.json".into()), ..Default::default() };
        let s = sources(&info, &[m.to_string(), "http://plain.example.org/".into()], Some(&log));
        let names: Vec<&str> = s.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["GitHub", "mirror m.example.org", "release log, peer.example.net"]);
        assert_eq!(s[0].expect_version.as_deref(), Some("0.3.5"));
        assert_eq!(s[1].manifest_url, "https://m.example.org/r/latest.json");
        assert_eq!(s[2].manifest_url, "https://peer.example.net/leyline/launcher-v0.3.5/latest.json");
        assert_eq!(s[2].manifest_sha256.as_deref(), Some("cd".repeat(32).as_str()));
        // GitHub down (no release found): our mirror and the log still offer the update
        let off = UpdateInfo { current: "0.3.1".into(), error: Some("offline".into()), ..Default::default() };
        assert_eq!(sources(&off, &[m.to_string()], Some(&log)).len(), 2);
        // nothing above the running version in the log: no log source
        let new = UpdateInfo { current: "0.3.5".into(), ..Default::default() };
        assert_eq!(sources(&new, &[], Some(&log)).len(), 0);
        // the shipped mirrors are all https folders
        assert!(crate::branding::get().updates.mirrors.iter().all(|m| net::plain_https(m) && m.ends_with('/')));
    }

    #[test]
    fn platform_keys_and_the_public_key() {
        assert_eq!(platform_key("windows", "x86_64"), "windows-x86_64");
        assert_eq!(platform_key("macos", "aarch64"), "darwin-aarch64");
        assert_eq!(platform_key("linux", "x86_64"), "linux-x86_64");
        assert!(PLATFORM_KEYS.contains(&this_platform().as_str()) || cfg!(not(target_arch = "x86_64")));
        let conf: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let pk = conf["plugins"]["updater"]["pubkey"].as_str().unwrap();
        // The repo ships the placeholder until "setup\Turn on launcher updates.bat" writes the real key.
        assert!(pk.starts_with("TODO") || pubkey_ok(pk), "{pk}");
        assert!(!pubkey_ok("TODO(owner): run Turn on launcher updates.bat"));
        assert!(pubkey_ok(&"dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6".repeat(3)));
        assert_eq!(conf["plugins"]["updater"]["windows"]["installMode"], "passive");
        assert!(conf["bundle"]["createUpdaterArtifacts"].is_null() || conf["bundle"]["createUpdaterArtifacts"] == false,
            "CI turns updater artifacts on only when the signing key is there");
    }
}
