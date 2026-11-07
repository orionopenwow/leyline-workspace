//! The settings file: client profiles, the chosen realm and the Wine setting, as JSON under the OS app-data dir.
//!
//! Saves are atomic (write a temp file in the same folder, flush it, rename over the old one), so a crash or
//! power cut leaves either the old file or the new one, never half of one. A file that will not parse is kept
//! aside as settings.json.broken and Leyline starts from defaults rather than refusing to open.

use crate::clients::ClientProfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "settings.json";
/// 2 (2 Nov): `hybrid_login` on by default; a file from before has it on once (it was only ever off by default).
pub const SCHEMA: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wine {
    /// Launch through Wine (macOS and Linux). Ignored on Windows.
    pub enabled: bool,
    /// The wine command, such as `wine`, `wine64` or a full path.
    pub command: String,
    /// Optional WINEPREFIX. Empty means Wine's default.
    pub prefix: String,
}

impl Default for Wine {
    fn default() -> Self {
        Wine { enabled: !cfg!(windows), command: "wine".into(), prefix: String::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    pub clients: Vec<ClientProfile>,
    pub selected_client: Option<String>,
    /// The realm address chosen on the Play page.
    pub selected_realm: String,
    pub wine: Wine,
    /// The host kit folder chosen on the Host page (empty: none yet).
    pub host_kit: String,
    /// Mods (manifest keys such as `mod:fps-fix`) whose native code the player OK'd once. Stored here, in the
    /// launcher's own folder, so the notice is shown once per mod, for every client.
    pub native_ok: Vec<String>,
    /// Which Leyline releases to update to: stable (releases) or beta (pre-releases too).
    pub update_channel: crate::update::Channel,
    /// After a catalog install, tell the hub "one more install" (anonymous: the mod id, the build and a fresh
    /// random id; rankings.rs). On unless the player turns it off.
    pub count_installs: bool,
    /// The hybrid client (hybrid.rs, docs/HYBRID-CLIENT.md): PLAY on the official realm and Hero Key sign-in hand
    /// the login to the game's login screen through Leyline's patch archive, so the player types nothing and lands at
    /// character select. On by default since 2 Nov (owner: "skipping to character select after realm"); it only
    /// does anything in a folder that qualifies (hybrid::status: the changed login screens there, the slot free or
    /// ours, the game never refused it), else the start is the plain one.
    pub hybrid_login: bool,
    /// The launcher's guide (ui/guide.js): the first-run tour ("tour") and each one-time encouragement line
    /// ("hosted", "friend", "mod") once shown. Kept here so each shows once, never again after a restart.
    pub seen: Vec<String>,
    /// Settings › "Hide the sign-in behind Leyline's screen" (crate::cover): while the launcher types the sign-in at
    /// the game's login screen (the official realm on a stock game, Your Den), Leyline's "Logging you in…" screen
    /// covers the game until character select (or the Den's loading screen). On by default; a file without it: on.
    pub signin_cover: bool,
    /// Leyline Solo's sign-in (host/classicsign.rs, docs/LEYLINE-SOLO.md): "prefill" (the default: the account name
    /// filled in, the password typed once) or "addon" (our sign-in add-on in the game folder: opt-in,
    /// TODO(owner) approval of copying the login screen's file list).
    pub solo_signin: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SCHEMA,
            clients: Vec::new(),
            selected_client: None,
            selected_realm: crate::OFFICIAL_REALM.into(),
            wine: Wine::default(),
            host_kit: String::new(),
            native_ok: Vec::new(),
            update_channel: crate::update::Channel::Stable,
            count_installs: true,
            hybrid_login: true,
            seen: Vec::new(),
            signin_cover: true,
            solo_signin: "prefill".into(),
        }
    }
}

impl Settings {
    pub fn client(&self, id: &str) -> Option<&ClientProfile> {
        self.clients.iter().find(|c| c.id == id)
    }
    pub fn client_mut(&mut self, id: &str) -> Option<&mut ClientProfile> {
        self.clients.iter_mut().find(|c| c.id == id)
    }
    /// The player OK'd native code for this mod.
    pub fn native_allowed(&self, key: &str) -> bool {
        self.native_ok.iter().any(|k| k == key)
    }
    /// Mark a guide key as shown (ui/guide.js). False for a key that isn't one: lower-case letters, digits and -,
    /// up to 32, and at most 64 keys kept.
    pub fn mark_seen(&mut self, key: &str) -> bool {
        let ok = !key.is_empty() && key.len() <= 32 && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !ok {
            return false;
        }
        if !self.seen.iter().any(|k| k == key) && self.seen.len() < 64 {
            self.seen.push(key.to_string());
        }
        true
    }
    pub fn remove_client(&mut self, id: &str) -> bool {
        let n = self.clients.len();
        self.clients.retain(|c| c.id != id);
        if self.selected_client.as_deref() == Some(id) {
            self.selected_client = self.clients.first().map(|c| c.id.clone());
        }
        self.clients.len() != n
    }
}

/// Another data folder for this run: an absolute path in `LEYLINE_APP_DIR` (the first-run test's fresh, empty one,
/// docs/FIRST-RUN-TEST.md; tools/den-check reads the same name). Unset, empty or relative: the normal folder.
pub const APP_DIR_ENV: &str = "LEYLINE_APP_DIR";

/// Leyline's data folder: %APPDATA%\Leyline, ~/Library/Application Support/Leyline, ~/.local/share/Leyline, or
/// [`APP_DIR_ENV`] when it is set.
pub fn app_dir() -> PathBuf {
    app_dir_with(std::env::var_os(APP_DIR_ENV))
}

/// [`app_dir`] with the override's value (None: unset).
pub fn app_dir_with(over: Option<std::ffi::OsString>) -> PathBuf {
    match over.map(PathBuf::from) {
        Some(p) if p.is_absolute() => p,
        _ => dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("Leyline"),
    }
}

/// Write `bytes` to `path` atomically.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // On Windows rename replaces an existing file too (MoveFileEx with REPLACE_EXISTING).
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Load the settings in `dir`. A missing file gives defaults; a broken one is moved aside, then defaults.
pub fn load(dir: &Path) -> Settings {
    let p = dir.join(FILE_NAME);
    match fs::read(&p) {
        Ok(b) => match serde_json::from_slice::<Settings>(&b) {
            Ok(mut s) => {
                // names saved before the rule (a folder named after the game) are cleaned on load
                for c in &mut s.clients {
                    c.name = crate::clients::tidy_name(&crate::clients::brandless(&c.name), c.build);
                }
                // the official realm saved under its old name (before the move to leylinelauncher.com) is the
                // official realm still: shown and chosen as such (addresses.rs)
                if crate::addresses::is_official_realm(&s.selected_realm) {
                    s.selected_realm = crate::OFFICIAL_REALM.into();
                }
                migrate(&mut s);
                s
            }
            Err(_) => {
                let _ = fs::rename(&p, dir.join(format!("{FILE_NAME}.broken")));
                Settings::default()
            }
        },
        Err(_) => Settings::default(),
    }
}

/// A file saved by an older Leyline, brought up to SCHEMA. To 2: the sign-in switch on (it was off by default and
/// had no other way to be on before; turned off after this it stays off).
fn migrate(s: &mut Settings) {
    if s.schema < 2 {
        s.hybrid_login = true;
    }
    s.schema = s.schema.max(SCHEMA);
}

pub fn save(dir: &Path, s: &Settings) -> io::Result<()> {
    let mut b = serde_json::to_vec_pretty(s).map_err(io::Error::other)?;
    b.push(b'\n');
    write_atomic(&dir.join(FILE_NAME), &b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::Build;

    #[test]
    fn app_dir_override_only_when_absolute() {
        let fresh = std::env::temp_dir().join("leyline-first-run");
        assert_eq!(app_dir_with(Some(fresh.clone().into_os_string())), fresh);
        let normal = app_dir_with(None);
        assert!(normal.ends_with("Leyline"));
        assert_eq!(app_dir_with(Some("relative/dir".into())), normal);
        assert_eq!(app_dir_with(Some("".into())), normal);
    }

    fn profile() -> ClientProfile {
        ClientProfile {
            id: "c335a-1".into(),
            name: "Client".into(),
            path: "/games/client".into(),
            build: Build::V335a,
            realmlist: "play.openvanilla.app".into(),
            locale: Some("enUS".into()),
            exe_version: Some("3.3.5.12340".into()),
            exe: None,
        }
    }

    #[test]
    fn round_trip() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load(d.path()), Settings::default());
        let mut s = Settings::default();
        s.clients.push(profile());
        s.selected_client = Some("c335a-1".into());
        s.wine.command = "wine64".into();
        s.native_ok.push("mod:fps-fix".into());
        s.update_channel = crate::update::Channel::Beta;
        save(d.path(), &s).unwrap();
        assert_eq!(load(d.path()), s);
        assert!(load(d.path()).native_allowed("mod:fps-fix") && !s.native_allowed("mod:other"));
        let text = fs::read_to_string(d.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("\"build\": \"3.3.5a\"") && text.contains("\"update_channel\": \"beta\""));
        // No temp file left behind.
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(FILE_NAME), b"{ not json").unwrap();
        assert_eq!(load(d.path()), Settings::default());
        assert!(d.path().join("settings.json.broken").exists());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(FILE_NAME), br#"{"selected_realm":"x.example.org"}"#).unwrap();
        let s = load(d.path());
        assert_eq!(s.selected_realm, "x.example.org");
        assert_eq!(s.wine.command, "wine");
    }

    #[test]
    fn signing_in_by_itself_is_on_by_default_and_stays_off_once_turned_off() {
        let d = tempfile::tempdir().unwrap();
        assert!(Settings::default().hybrid_login);
        // a file from before 2 Nov: the switch was off only because that was the default
        fs::write(d.path().join(FILE_NAME), br#"{"schema":1,"hybrid_login":false,"selected_realm":"x.example.org"}"#).unwrap();
        let s = load(d.path());
        assert!(s.hybrid_login && s.schema == SCHEMA);
        // turned off after that: kept off
        let off = Settings { hybrid_login: false, ..s };
        save(d.path(), &off).unwrap();
        assert!(!load(d.path()).hybrid_login);
        fs::write(d.path().join(FILE_NAME), br#"{"selected_realm":"x.example.org"}"#).unwrap();
        assert!(load(d.path()).hybrid_login, "no switch saved: on");
    }

    #[test]
    fn the_sign_in_cover_is_on_by_default_and_stays_off_once_turned_off() {
        let d = tempfile::tempdir().unwrap();
        assert!(Settings::default().signin_cover);
        fs::write(d.path().join(FILE_NAME), br#"{"selected_realm":"x.example.org"}"#).unwrap();
        let s = load(d.path());
        assert!(s.signin_cover, "a file from before the switch: on");
        save(d.path(), &Settings { signin_cover: false, ..s }).unwrap();
        assert!(!load(d.path()).signin_cover);
    }

    #[test]
    fn remove_client_moves_the_selection() {
        let mut s = Settings::default();
        let mut b = profile();
        b.id = "c1121-1".into();
        s.clients = vec![profile(), b];
        s.selected_client = Some("c335a-1".into());
        assert!(s.remove_client("c335a-1"));
        assert_eq!(s.selected_client.as_deref(), Some("c1121-1"));
        assert!(!s.remove_client("nope"));
    }

    #[test]
    fn guide_keys_are_kept_once_and_checked() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Settings::default();
        assert!(s.seen.is_empty());
        assert!(s.mark_seen("tour") && s.mark_seen("tour") && s.mark_seen("first-mod"));
        assert_eq!(s.seen, vec!["tour".to_string(), "first-mod".to_string()]);
        for bad in ["", "Tour", "../x", "a b", &"x".repeat(33)] {
            assert!(!s.mark_seen(bad), "{bad}");
        }
        save(d.path(), &s).unwrap();
        assert_eq!(load(d.path()).seen, s.seen);
        // a settings file from before the guide: nothing seen yet
        fs::write(d.path().join(FILE_NAME), br#"{"selected_realm":"x.example.org"}"#).unwrap();
        assert!(load(d.path()).seen.is_empty());
    }

    #[test]
    fn atomic_write_replaces() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/f.txt");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
    }
}
