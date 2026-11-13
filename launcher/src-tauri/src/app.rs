//! The desktop app: Tauri commands over the core modules. The web UI in `launcher/ui/` calls these.
//!
//! The UI never hands the core a URL to fetch or a path to write: installs name a catalog project (looked up in
//! the cached catalog) or a file the player picked or dropped; links open only from the fixed list in `LINKS`
//! or from the set of addresses the core itself handed the UI (catalog credits, realm pages, the release page).

use crate::catalog::{self, Catalog};
use crate::clients::{self, ClientProfile, Detection};
use crate::host::{self, HostOs, HostStatus, KitInfo};
use crate::mods::{self, Installed, InstalledView, Removed};
use crate::net::{self, Listing};
use crate::realms::{self, RealmList};
use crate::settings::{self, Settings, Wine};
use crate::update::{self, Channel, UpdateInfo};
use crate::{launch, status, BROWSER_PLAY_URL, OFFICIAL_REALM};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

/// pub(crate): the Leyline addons commands (addons.rs) share the settings, the folder and the mod lock.
pub(crate) struct Core {
    pub(crate) dir: PathBuf,
    settings: Mutex<Settings>,
    /// Addresses the core gave the UI, which `open_url` may open.
    known_urls: Mutex<HashSet<String>>,
    /// One mod operation at a time.
    pub(crate) mods_lock: Arc<Mutex<()>>,
    /// Games Leyline started: a launcher update never restarts while one of them runs.
    games: Mutex<Vec<Child>>,
    /// The launcher's own update: what the last check said, and a downloaded update waiting for "Restart".
    upd: Mutex<UpdState>,
    /// A `leyline://` link Leyline was started with or was handed while open, until the UI takes it (deeplink.rs).
    deep_link: Mutex<crate::deeplink::Inbox>,
}

impl Core {
    pub(crate) fn update<T>(&self, f: impl FnOnce(&mut Settings) -> Result<T, String>) -> Result<T, String> {
        let mut s = self.settings.lock().map_err(|_| "settings lock poisoned")?;
        let out = f(&mut s)?;
        settings::save(&self.dir, &s).map_err(|e| format!("Could not save settings: {e}"))?;
        Ok(out)
    }
    pub(crate) fn snapshot(&self) -> Settings {
        self.settings.lock().map(|s| s.clone()).unwrap_or_default()
    }
    pub(crate) fn client(&self, id: &str) -> Result<ClientProfile, String> {
        self.snapshot().client(id).cloned().ok_or_else(|| "Pick a game client first.".into())
    }
    fn allow(&self, urls: impl IntoIterator<Item = String>) {
        if let Ok(mut k) = self.known_urls.lock() {
            for u in urls {
                if net::plain_https(&u) && k.len() < 20_000 {
                    k.insert(u);
                }
            }
        }
    }
    fn kit(&self) -> Option<PathBuf> {
        Some(self.snapshot().host_kit).filter(|k| !k.is_empty()).map(PathBuf::from)
    }
    /// A game Leyline started is still running.
    fn game_running(&self) -> bool {
        let Ok(mut g) = self.games.lock() else { return true };
        g.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
        !g.is_empty()
    }
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())? }
}

#[derive(Serialize)]
struct AppState {
    version: &'static str,
    settings: Settings,
    official_realm: &'static str,
    browser_url: &'static str,
    windows: bool,
    data_dir: String,
}

/// What the UI runs on (ui/platform.js): a computer here; the phone app answers the same command (mobile.rs).
#[tauri::command]
fn platform() -> crate::Platform {
    crate::platform()
}

#[tauri::command]
fn get_state(core: State<Core>) -> AppState {
    AppState {
        version: env!("CARGO_PKG_VERSION"),
        settings: core.snapshot(),
        official_realm: OFFICIAL_REALM,
        browser_url: BROWSER_PLAY_URL,
        windows: cfg!(windows),
        data_dir: core.dir.display().to_string(),
    }
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    pick(app, "Choose your game client folder", false, false).await
}

async fn pick(app: AppHandle, title: &'static str, file: bool, mods_filter: bool) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = app.dialog().file().set_title(title);
        if file {
            let d = if mods_filter { d.add_filter("Mods (.zip, .MPQ, .dll, .asi)", &["zip", "mpq", "MPQ", "dll", "DLL", "asi"]) } else { d };
            d.blocking_pick_file()
        } else {
            d.blocking_pick_folder()
        }
    })
    .await
    .ok()
    .flatten()
    .and_then(|p| p.into_path().ok())
    .map(|p| p.display().to_string())
}

/// Settings › Save my Leyline network key: the key file written where the player picks (never shown, never logged).
/// None: the save was cancelled.
#[tauri::command]
async fn peer_key_export(app: AppHandle) -> Result<Option<String>, String> {
    let text = crate::identity::export_text(&settings::app_dir())?;
    let to = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_title("Save your Leyline network key").set_file_name("leyline-network-key.txt").add_filter("Text", &["txt"]).blocking_save_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|p| p.into_path().ok());
    let Some(to) = to else { return Ok(None) };
    std::fs::write(&to, text.as_bytes()).map_err(|e| format!("The key wasn't saved: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&to, std::fs::Permissions::from_mode(0o600));
    }
    Ok(Some("Saved. Keep that file private: whoever has it is you to your friends. Bring it back here on a new PC or after a reinstall.".into()))
}

/// Settings › Bring back my Leyline network key: a saved key file picked, this PC becomes that identity (the key it
/// had is kept aside). None: cancelled.
#[tauri::command]
async fn peer_key_import(app: AppHandle) -> Result<Option<String>, String> {
    let Some(from) = pick(app, "Pick your saved Leyline network key", true, false).await else { return Ok(None) };
    blocking(move || {
        let meta = std::fs::metadata(&from).map_err(|e| e.to_string())?;
        if meta.len() > 4096 {
            return Err("That file isn't a saved Leyline network key.".to_string());
        }
        let text = std::fs::read_to_string(&from).map_err(|_| "That file isn't a saved Leyline network key.".to_string())?;
        let short = crate::identity::import(&settings::app_dir(), &text)?;
        Ok(Some(format!("Done: this PC is Hero {short} again to your friends. Restart Leyline to finish. The key it had is kept aside in Leyline's folder.")))
    })
    .await
}

#[tauri::command]
fn detect_client(path: String) -> Result<Detection, String> {
    clients::detect(&PathBuf::from(path)).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Added {
    profile: ClientProfile,
    detection: Detection,
}

#[tauri::command]
fn add_client(core: State<Core>, path: String) -> Result<Added, String> {
    let path = PathBuf::from(path.trim());
    let detection = clients::detect(&path).map_err(|e| e.to_string())?;
    add_profile(&core, path, detection)
}

/// A game folder added to the settings (by hand, or found by `world_find_folder`): the same profile either way.
fn add_profile(core: &Core, path: PathBuf, detection: Detection) -> Result<Added, String> {
    core.update(|s| {
        if s.clients.iter().any(|c| c.path == path) {
            return Err("That folder is already in your clients.".into());
        }
        let profile = clients::new_profile(&path, &detection, &s.clients);
        s.clients.push(profile.clone());
        if s.selected_client.is_none() {
            s.selected_client = Some(profile.id.clone());
        }
        Ok(Added { profile, detection })
    })
}

#[tauri::command]
fn check_client(core: State<Core>, id: String) -> Result<Detection, String> {
    let p = core.client(&id)?;
    clients::detect(&p.path).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_client(core: State<Core>, id: String, name: String) -> Result<(), String> {
    let name = name.trim().chars().take(60).collect::<String>();
    if name.is_empty() {
        return Err("A name is needed.".into());
    }
    core.update(|s| {
        s.client_mut(&id).ok_or("No such client.")?.name = name;
        Ok(())
    })
}

#[tauri::command]
fn remove_client(core: State<Core>, id: String) -> Result<(), String> {
    core.update(|s| s.remove_client(&id).then_some(()).ok_or_else(|| "No such client.".into()))
}

#[tauri::command]
fn select(core: State<Core>, client: Option<String>, realm: String) -> Result<(), String> {
    core.update(|s| {
        s.selected_client = client;
        s.selected_realm = realm.trim().to_string();
        Ok(())
    })
}

#[tauri::command]
fn set_wine(core: State<Core>, wine: Wine) -> Result<(), String> {
    core.update(|s| {
        s.wine = wine;
        Ok(())
    })
}

/// Point the client at `realm` (its realm file: realmlist.wtf or Config.wtf, by build), remember it,
/// optionally start it.
fn point_and_play(core: &Core, client: &str, realm: &str, start: bool) -> Result<String, String> {
    point_and_play_with(core, client, realm, start, None, false).map(|(m, _)| m)
}

/// `point_and_play`, plus the hybrid client (docs/HYBRID-CLIENT.md): with Settings' hybrid_login on and a 3.3.5a
/// client, `session` is handed to the login screen through Leyline's patch archive; without one, a session left
/// from an earlier start is taken out first, so a plain start never signs in by itself. Returns whether the
/// session went in (Err text in the second value when it didn't, with the switch on). `local`: the session is the
/// local Den's fixed test login (host/lite.rs, no secret, never leaves this PC) or a friend's Den login from their
/// invite (one friend, hours only, taken out of the game folder when the game closes), handed over whatever the
/// switch says.
fn point_and_play_with(
    core: &Core,
    client: &str,
    realm: &str,
    start: bool,
    session: Option<crate::hybrid::Session>,
    local: bool,
) -> Result<(String, Result<(), String>), String> {
    let realm = realm.trim().to_string();
    if !clients::valid_realm_address(&realm) {
        return Err("That realm address does not look right (a host name, optionally :port).".into());
    }
    // A listed realm with a known build takes only clients of that build.
    if let Some(r) = realms::cached(&core.dir).into_iter().find(|r| r.address.as_deref() == Some(realm.as_str())) {
        let p = core.client(client)?;
        if r.build.is_some_and(|b| b != p.build) {
            let s = core.snapshot();
            return Err(realms::pick_client(&r, &s.clients, Some(client))
                // the era, never a version number in a sentence (players' words; review 4 Nov)
                .map(|c| format!("{} needs a {} game folder: pick {} for it.", r.name, c.build.era().unwrap_or("different"), c.name))
                .unwrap_or_else(|e| e));
        }
    }
    let (profile, wine) = core.update(|s| {
        s.selected_client = Some(client.to_string());
        s.selected_realm = realm.clone();
        let c = s.client_mut(client).ok_or("Pick a game client first.")?;
        c.realmlist = realm.clone();
        Ok((c.clone(), s.wine.clone()))
    })?;
    // The official realm on either of its names: the new one, or the old one on a computer that cannot resolve the
    // new name yet (addresses.rs). Both reach the same server.
    let written = if crate::addresses::is_official_realm(&realm) { crate::addresses::official_realm().to_string() } else { realm.clone() };
    clients::write_realmlist(&profile, &written).map_err(|e| format!("Could not write realmlist.wtf: {e}"))?;
    let mut hybrid: Result<(), String> = Err(String::new());
    if !start {
        return Ok((format!("{} now points at {realm}.", profile.name), hybrid));
    }
    // anywhere but a Den: the Den's account and realm name (den_launch) out of Config.wtf, so the switch is clean
    if !local && realm != crate::host::lite::ADDRESS {
        let _ = crate::herokey::clear_login(&profile.path, crate::host::lite::ACCOUNT, crate::host::lite::REALM);
        // a start where Leyline signs nobody in (a community realm, the official realm signed out): the account names
        // Leyline wrote for earlier starts come out too, so the login screen never shows the player's official account
        // (or a Den's) on another realm (review 4 Nov). A start that signs in wrote its own name just before this. The
        // Docker Den and realms on this PC keep the name the Den page filled in (den_prefill_account).
        if session.is_none() && !on_this_pc(&realm) {
            let _ = crate::herokey::clear_written_names(&profile.path);
        }
    }
    crate::gfx::prepare_launch(&profile)?;
    let hybrid_on = (core.snapshot().hybrid_login || local) && profile.build == clients::Build::V335a;
    match &session {
        Some(s) if hybrid_on => hybrid = crate::hybrid::write(&profile.path, Some(s)).map(|_| ()),
        // the switch off: no sign-in from an earlier start stays in the folder either (2 Nov)
        _ => {
            let _ = crate::hybrid::scrub(&profile.path);
        }
    }
    // Your Den's ground (host/terrain.rs): patch-T from the player's own files plus the Den's delta before a Den
    // starts; taken away before anywhere else. Never stops the start: a problem is said after it.
    let mut ground_note = String::new();
    if profile.build == clients::Build::V335a {
        let r = match den_ground_root(core, &realm) {
            Some(root) => crate::host::terrain::prepare_client(&profile.path, &root).map(|_| ()),
            None => crate::host::terrain::remove_patch(&profile.path).map(|_| ()),
        };
        if let Err(e) = r {
            ground_note = format!(" (Your Den's ground: {e})");
        }
        // The official realm's race and class choices (crate::combos, docs/RACE-CLASS-COMBOS.md): the create screen's
        // tables from the player's own files, in patch-Y, before PLAY there; taken away before anywhere else. Never
        // stops the start either.
        let preset = if crate::addresses::is_official_realm(&realm) { crate::combos::official_preset(&core.dir) } else { crate::combos::Preset::Off };
        if let Err(e) = crate::combos::prepare_client(&profile.path, preset) {
            ground_note.push_str(&format!(" (The realm's race and class choices: {e})"));
        }
    }
    let plan = launch::plan(&profile, &wine, launch::Os::current())?;
    let child = launch::run(&plan).inspect_err(|_| {
        let _ = crate::hybrid::scrub(&profile.path);
    })?;
    LAST_GAME_PID.store(child.id(), std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut g) = core.games.lock() {
        g.push(child);
    }
    if hybrid.is_ok() {
        scrub_when_closed(profile.path.clone());
    }
    Ok((format!("Started {} on {realm}.{ground_note}", profile.name), hybrid))
}

/// A realm on this PC (the Docker Den, the player's own hosted realm): `127.x`, `localhost`, or the Docker Den's
/// knocked address.
fn on_this_pc(realm: &str) -> bool {
    let host = realm.rsplit_once(':').map(|(h, _)| h).unwrap_or(realm);
    host.starts_with("127.") || host.eq_ignore_ascii_case("localhost") || DEN_ADDRESS.lock().ok().and_then(|g| g.clone()).as_deref() == Some(realm)
}

/// The Docker Den's address on this PC, once its own invite was knocked (join_invite): its plays get the Den's ground.
static DEN_ADDRESS: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Where the Den's ground is when `realm` is a Den: Den Lite on this PC, or the Host pack's Docker Den.
fn den_ground_root(core: &Core, realm: &str) -> Option<PathBuf> {
    use crate::host::lite::DenLite;
    if realm == crate::host::lite::ADDRESS && crate::host::lite::Binary::find(&crate::settings::app_dir()).status().running {
        return Some(crate::host::lite::root(&crate::settings::app_dir()));
    }
    let den = DEN_ADDRESS.lock().ok().and_then(|g| g.clone());
    if den.as_deref() == Some(realm) {
        return core.kit();
    }
    None
}

/// Leyline's in-game addons brought up to date right before a PLAY on a 3.3.5a client (crate::addons::
/// prepare_for_play; 12 Nov: updates only on launcher start never reached the owner's game folder). Quick (the last
/// good listing when fresh, else 3 s for the release), never an error, skipped while another install holds the mod
/// lock. What it updated is the play bar's "In-game add-ons updated" (addons_play_note). `install_missing`: put the
/// pack in where it isn't (the first Single player setup).
fn addons_before_launch_now(core: &Core, client: &str, install_missing: bool) {
    let Ok(p) = core.client(client) else { return };
    // the page checked them a moment ago (addons.js beforePlay): one check per Play
    if p.build != clients::Build::V335a || (!install_missing && crate::addons::checked_recently(&p.id)) {
        return;
    }
    let Ok(_g) = core.mods_lock.try_lock() else { return };
    let r = crate::addons::prepare_for_play(&core.dir, &p, crate::addons::DESCRIPTOR_URL, install_missing);
    crate::addons::note_play(&r);
}

/// `addons_before_launch_now` off the async runtime's own threads.
async fn addons_before_launch(core: &Core, client: &str, install_missing: bool) {
    let Ok(p) = core.client(client) else { return };
    if p.build != clients::Build::V335a || (!install_missing && crate::addons::checked_recently(&p.id)) {
        return;
    }
    let (dir, lock) = (core.dir.clone(), core.mods_lock.clone());
    let _ = blocking(move || {
        let Ok(_g) = lock.try_lock() else { return Ok(()) };
        let r = crate::addons::prepare_for_play(&dir, &p, crate::addons::DESCRIPTOR_URL, install_missing);
        crate::addons::note_play(&r);
        Ok(())
    })
    .await;
}

/// Point the client at the realm, then start it. On the official realm, signed in on Home, the game signs in by
/// itself and opens at character select (`play_official`).
#[tauri::command]
async fn play(app: AppHandle, core: State<'_, Core>, client: String, realm: String) -> Result<String, String> {
    addons_before_launch(&core, &client, false).await;
    if crate::addresses::is_official_realm(realm.trim()) {
        return play_official(&core, &client, &realm, Some(&app)).await;
    }
    point_and_play(&core, &client, &realm, true)
}

/// PLAY on the official realm (owner 2 Nov: "goes to the loginscreen instead of skipping to character select after
/// realm"), the way Your Den does: the player's own login from the website (crate::herokey::official_login, the
/// launcher's token; nothing asked when signed out), its account name in Config.wtf, and the login handed to the
/// login screen for this one start (crate::hybrid, `enter: false`: it stops at character select). Where that can't
/// be (a folder without the changed login screens, the switch off), the account name is still filled in. Never stops
/// the start: a failed sign-in is said after it. `app`: the launcher window's app, for the sign-in cover over the keys
/// (crate::cover; none from the command line).
async fn play_official(core: &Core, client: &str, realm: &str, app: Option<&AppHandle>) -> Result<String, String> {
    let profile = core.client(client)?;
    let mut login: Result<crate::herokey::GameLogin, String> = Err("signed_out".into());
    if profile.build == clients::Build::V335a {
        let dir = core.dir.clone();
        login = blocking(move || crate::herokey::official_login(&dir)).await;
    }
    let session = login.as_ref().ok().map(|l| {
        let _ = crate::herokey::set_account_name(&profile.path, &l.user);
        crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: false, pick: None, make: None }
    });
    let pass = login.as_ref().ok().map(|l| l.pass.clone());
    let name = login.as_ref().ok().map(|l| l.user.clone()).unwrap_or_default();
    let asked = login.map(|_| ());
    let (message, mut hybrid) = point_and_play_with(core, client, realm, true, session, false)?;
    // a stock game (no Forever UI's login screens): the launcher's keys sign in instead, behind Leyline's cover
    if let (Err(_), Some(pass)) = (&hybrid, pass) {
        let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
        if pid != 0 && cfg!(windows) {
            let cover = match app {
                Some(app) if core.snapshot().signin_cover => cover_start(app, pid, crate::cover::Mode::Official, &name, None),
                _ => None,
            };
            keys_sign_in(pid, pass, cover);
            hybrid = Ok(());
        }
    }
    Ok(message + &official_note(&asked, &hybrid))
}

/// How long after the game's window shows the login screen takes to draw and take keys (the owner's PC: the window at
/// once, the login screen a few seconds later; client-smoke 3 Nov).
const KEYS_SETTLE: std::time::Duration = std::time::Duration::from_secs(7);

/// The official realm on a stock game (owner 3 Nov 00:38: "as smooth as possible for the end consumer"): the stock game
/// refuses changed login screen files, so instead of them the launcher types the sign-in for this start at the login
/// screen, as Your Den's keys do (crate::denclick, posted window messages, no focus taken): the account name is
/// already filled in (Config.wtf), so the password and Enter. One try; if the login screen wasn't ready, the player
/// is on the normal login screen with the name filled in. The password is only in this thread's memory. `cover`: the
/// sign-in cover's signals (crate::cover), told when the keys went or didn't.
fn keys_sign_in(pid: u32, pass: String, cover: Option<Arc<crate::cover::Signals>>) -> std::thread::JoinHandle<Result<(), String>> {
    std::thread::spawn(move || {
        let r = keys_sign_in_now(pid, &pass);
        if let Some(c) = cover {
            if r.is_ok() { c.keys_sent() } else { c.keys_failed() }
        }
        r
    })
}

fn keys_sign_in_now(pid: u32, pass: &str) -> Result<(), String> {
    use crate::denclick::{Game, Key, Process};
    let mut g = Process(pid);
    let t0 = std::time::Instant::now();
    while !g.window() {
        if !g.alive() || t0.elapsed() > std::time::Duration::from_secs(60) {
            return Err("the game window didn't open".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    std::thread::sleep(KEYS_SETTLE);
    if !g.alive() {
        return Err("the game closed before the login screen".into());
    }
    if g.send(Key::Password, pass) { Ok(()) } else { Err("the game window didn't take the keys".into()) }
}

/// The sign-in cover (crate::cover, owner 3 Nov): Leyline's "Logging you in…" screen over the game's window while the
/// launcher signs in, from the moment the window opens until character select (or the Den's loading screen), and
/// gone at once when anything goes another way. Windows only (where the keys are); its own thread, so the start
/// never waits on it. Returns the signals the signing-in side sets. `den`: Den Lite's Dens folder and the start (unix
/// ms) whose milestones it follows.
fn cover_start(app: &AppHandle, pid: u32, mode: crate::cover::Mode, name: &str, den: Option<(PathBuf, u64)>) -> Option<Arc<crate::cover::Signals>> {
    if !cfg!(windows) || pid == 0 {
        return None;
    }
    let sig = Arc::new(crate::cover::Signals::default());
    let id = crate::cover::begin(mode, crate::cover::title(mode, name));
    let (app, s) = (app.clone(), sig.clone());
    std::thread::spawn(move || cover_watch(app, pid, mode, den, s, id));
    Some(sig)
}

#[cfg(windows)]
fn cover_watch(app: AppHandle, pid: u32, mode: crate::cover::Mode, den: Option<(PathBuf, u64)>, sig: Arc<crate::cover::Signals>, id: u64) {
    use crate::cover::{self, win};
    let log = settings::app_dir().join("signin-cover.log");
    let say = |text: String| {
        let _ = std::fs::write(&log, text);
    };
    let w = match cover_window(&app) {
        Ok(w) => w,
        Err(e) => return say(format!("sign-in cover ({mode:?})\nno cover window: {e}\n")),
    };
    let hwnd = match w.hwnd() {
        Ok(h) => h.0 as isize,
        Err(e) => return say(format!("sign-in cover ({mode:?})\nno cover window: {e}\n")),
    };
    win::prepare(hwnd);
    // whatever happens in here (even a panic), the cover doesn't stay up (unless a newer start has it now)
    struct Down(isize, u64);
    impl Drop for Down {
        fn drop(&mut self) {
            if crate::cover::current() == self.1 {
                win::hide(self.0);
            }
        }
    }
    let down = Down(hwnd, id);
    let mut screen = win::Desktop { pid, cover: hwnd };
    let start = crate::denclick::now_ms();
    let den_ref = den.as_ref().map(|(d, t)| (d.as_path(), *t));
    let (end, steps) = cover::run(&mut screen, cover::Watch::new(mode, start), &sig, den_ref, id, &mut crate::denclick::now_ms, &mut |ms| {
        std::thread::sleep(std::time::Duration::from_millis(ms))
    });
    say(cover::log_text(mode, &end, &steps));
    drop(down);
    // made again for the next start: a hidden window left open would keep Leyline running after its main window
    // closes (a newer start that took it over destroys it when it is done)
    if cover::current() == id {
        let _ = w.destroy();
    }
}

#[cfg(not(windows))]
fn cover_watch(app: AppHandle, pid: u32, mode: crate::cover::Mode, den: Option<(PathBuf, u64)>, sig: Arc<crate::cover::Signals>, id: u64) {
    let _ = (app, pid, mode, den, sig, id);
}

/// The cover's window, made hidden when the start begins (its page loads while the game opens) and destroyed after it.
/// Undecorated, on top, off the taskbar, never focused (on Windows it is also made unable to take the keyboard:
/// cover::win::prepare), dark before its page draws.
#[cfg_attr(not(windows), allow(dead_code))]
fn cover_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(crate::cover::LABEL) {
        return Ok(w);
    }
    tauri::WebviewWindowBuilder::new(app, crate::cover::LABEL, tauri::WebviewUrl::App(crate::cover::PAGE.into()))
        .title("Leyline")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .background_color(tauri::window::Color(11, 13, 18, 255))
        .inner_size(960.0, 600.0)
        .build()
}

/// The cover's page (ui/cover.js): what to show.
#[tauri::command]
fn cover_state() -> crate::cover::View {
    crate::cover::view()
}

/// The cover's "Show the game" (or Esc on it): this start's cover goes, the game in front.
#[tauri::command]
fn cover_hide(id: u64) {
    crate::cover::dismiss(id);
}

/// The cover's page: where its "Show the game" link is (CSS pixels and the page's scale), so the cover takes clicks
/// there only and lets every other click through to the game (crate::cover::hit).
#[tauri::command]
fn cover_link(id: u64, x: f64, y: f64, w: f64, h: f64, scale: f64) {
    crate::cover::set_link(id, crate::cover::link_from_css(x, y, w, h, scale));
}

/// Settings › Hide the sign-in behind Leyline's screen.
#[tauri::command]
fn set_signin_cover(core: State<Core>, on: bool) -> Result<(), String> {
    core.update(|s| {
        s.signin_cover = on;
        Ok(())
    })
}

/// What PLAY on the official realm adds to "Started …": how the player gets past the login screen. `login`: the
/// website's login ("signed_out" when not signed in on Home); `hybrid`: whether the login screen got it.
fn official_note(login: &Result<(), String>, hybrid: &Result<(), String>) -> String {
    const PW: &str = "type your game password at the login screen (the website's Play page shows it, under your own game client).";
    match (login, hybrid) {
        (Ok(()), Ok(())) => " It signs you in by itself and opens at character select.".into(),
        (Ok(()), Err(e)) if e.is_empty() => format!(" Your account name is filled in: {PW}"),
        (Ok(()), Err(e)) => format!(" Your account name is filled in: {PW} (Signing in by itself didn't work here: {e})"),
        (Err(e), _) if e == "signed_out" => " Log in to Leyline and PLAY signs you in by itself.".into(),
        (Err(e), _) => format!(" Signing in by itself didn't work ({e}): {PW}"),
    }
}

/// Play on a realm from the Realms page: the official realm or a listed Beacon (its relay address). `client` is
/// the player's pick, and exactly that client starts: when its version can't play the realm, the reply says which
/// folder to pick (realms::pick_client; owner 2 Nov, no silent swap). The official realm signs in by itself as PLAY
/// does (`play_official`).
#[tauri::command]
async fn play_realm(app: AppHandle, core: State<'_, Core>, client: String, slug: String) -> Result<String, String> {
    let dir = core.dir.clone();
    let realm = realms::cached(&dir).into_iter().find(|r| r.slug == slug).ok_or("That realm is not on the list any more.")?;
    let client = {
        let s = core.snapshot();
        realms::pick_client(&realm, &s.clients, Some(&client))?.id.clone()
    };
    addons_before_launch(&core, &client, false).await;
    let r2 = realm.clone();
    let addr = blocking(move || realms::desktop_address(&r2, |relay| format!("https://{relay}"))).await?;
    let mut msg = if realm.official { play_official(&core, &client, &addr, Some(&app)).await? } else { point_and_play(&core, &client, &addr, true)? };
    if !realm.official && realm.state != "live" {
        msg.push_str(" The list says this realm is offline right now, so it may not answer.");
    }
    Ok(msg)
}

/// "Sign in with your Hero Key" on a community realm (the herokey_sso flag; docs/HERO-KEY-SSO.md): the website's
/// ticket for that realm, redeemed at its relay (the realm makes or finds this player's account and sets a one-time
/// login), the account name written into the client's Config.wtf, then the client pointed at the realm and started.
/// The one-time password comes back for the UI to show: the 3.3.5a client can't take one from outside.
#[tauri::command]
async fn play_realm_herokey(core: State<'_, Core>, client: String, slug: String) -> Result<crate::herokey::Login, String> {
    let dir = core.dir.clone();
    if !crate::p2p::flags::current(&dir).herokey_sso {
        return Err("Hero Key sign-in for community realms isn't on yet.".into());
    }
    let realm = realms::cached(&dir).into_iter().find(|r| r.slug == slug).ok_or("That realm is not on the list any more.")?;
    let rid = realm.realm_id.clone().filter(|_| !realm.official).ok_or("Hero Key sign-in is for community realms.")?;
    let client = {
        let s = core.snapshot();
        realms::pick_client(&realm, &s.clients, Some(&client))?.id.clone()
    };
    addons_before_launch(&core, &client, false).await;
    let d2 = dir.clone();
    let login = blocking(move || crate::herokey::sign_in(&d2, &rid)).await.map_err(|e| {
        if e == "signed_out" { "Log in to Leyline first: Log In, in the title bar.".to_string() } else { e }
    })?;
    let profile = core.client(&client)?;
    let _ = crate::herokey::set_account_name(&profile.path, &login.user);
    let mut login = login;
    let r2 = realm.clone();
    let addr = blocking(move || realms::desktop_address(&r2, |relay| format!("https://{relay}"))).await?;
    // The hybrid client (Settings' hybrid_login, docs/HYBRID-CLIENT.md): the login screen signs in by itself.
    let s = crate::hybrid::Session { user: login.user.clone(), pass: login.pass.clone(), realm: None, enter: false, pick: None, make: None };
    let (_, hybrid) = point_and_play_with(&core, &client, &addr, true, Some(s), false)?;
    match hybrid {
        Ok(()) => {
            login.auto = true;
            login.pass.clear(); // the game has it; the page has nothing to show
        }
        Err(e) => login.auto_why = e,
    }
    Ok(login)
}

/// Take the one-time login out of the hybrid archive as soon as the game lets go of it (crate::hybrid::scrub fails
/// while the game holds the archive open). The glue reads it at start, so the first try waits 90 s; then every
/// 15 s, for at most 12 hours (the login stopped working long before; this only tidies the file).
fn scrub_when_closed(client: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(90));
        for _ in 0..(12 * 240) {
            if crate::hybrid::scrub(&client).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(15));
        }
    });
}

/// Settings › the hybrid client (docs/HYBRID-CLIENT.md). Turning it off takes Leyline's archive out of every
/// client it is in (someone else's archive in that slot is never touched).
#[tauri::command]
fn set_hybrid_login(core: State<Core>, on: bool) -> Result<String, String> {
    let s = core.update(|s| {
        s.hybrid_login = on;
        Ok(s.clone())
    })?;
    if on {
        return Ok("The game signs in by itself on Wrath Unleashed and Hero Key realms.".into());
    }
    let mut n = 0;
    for c in &s.clients {
        if crate::hybrid::remove(&c.path)? {
            n += 1;
        }
    }
    Ok(format!("Off. Leyline's login archive taken out of {n} game folder(s)."))
}

/// What the hybrid client would do with this game folder (Forever UI there, the slot free or ours).
#[tauri::command]
fn hybrid_status(core: State<Core>, client: String) -> Result<crate::hybrid::Status, String> {
    Ok(crate::hybrid::status(&core.client(&client)?.path))
}

#[derive(Serialize)]
struct Joined {
    address: String,
    name: String,
    /// A friend's Den invite (host/litenet.rs): the game was started with the friend's own login.
    guest: Option<GuestJoin>,
    /// Which way in a Den invite used (host/denways.rs): "lan", "own" or "relay" ("" for a realm's invite), and
    /// what was tried before it.
    way: String,
    tried: Vec<String>,
}

#[derive(Serialize)]
struct GuestJoin {
    /// The game signs in by itself (the hybrid login file, one start only).
    auto: bool,
    account: String,
    /// To type at the login screen when it can't sign in by itself; empty otherwise.
    password: String,
    why: String,
    message: String,
}

/// A private realm's invite link: open its door for this PC (the relay's knock), then point the client at it.
#[tauri::command]
async fn join_invite(core: State<'_, Core>, client: String, link: String) -> Result<Joined, String> {
    core.client(&client)?;
    // a Den code (DEN-...: host/denhost.rs) is the same as its link, the relay way
    let link = crate::host::denhost::link_from_code(&link, realms::RELAY).unwrap_or(link);
    let (relay, rid, secret) = realms::parse_invite(&link).ok_or("That is not an invite link (https://<relay>/join/<realm>#<code>) or a Den code (DEN-...).")?;
    // a friend's Den (Den Lite) or single player world: the same network first, then the friend's own address, then
    // the relay (denways.rs)
    let guest = crate::host::litenet::guest_in(&link);
    let lasting = crate::host::worldfriends::marked(&link);
    if guest.is_some() || lasting {
        use crate::host::worldfriends as wf;
        let profile = core.client(&client)?;
        if profile.build != clients::Build::V335a {
            return Err(if lasting { "Your friend's world plays with a Wrath game folder: pick that one." } else { "Your friend's Den plays with a Wrath game folder: pick that one." }.into());
        }
        let (lan, at) = crate::host::denways::ways_in(&link);
        let lan_kept = lan.clone().unwrap_or_default();
        let (way, list, tried) = blocking(move || Ok(crate::host::denways::pick(lan.as_deref(), at.as_deref(), crate::host::denways::connects))).await?;
        let (relay2, rid2, secret2) = (relay.clone(), rid.clone(), secret.clone());
        let (address, name) = if way == "relay" {
            let tried_now = tried.clone();
            blocking(move || realms::knock(&format!("https://{relay2}"), &rid2, &secret2)).await.map_err(|e| {
                let before = if tried_now.is_empty() { String::new() } else { format!("{} ", tried_now.join(" ")) };
                format!("{before}The Leyline relay: {e}")
            })?
        } else {
            (list, if lasting { "Your friend's world".to_string() } else { "Your friend's Den".to_string() })
        };
        // a single player world (host/worldfriends.rs): this launcher's identity claims its lasting login there before
        // the game logs in, so the same characters are there every time; a world that doesn't answer claims (an older
        // Leyline) takes the invite's one-time login as before
        let (user, pass) = if lasting {
            let app = settings::app_dir();
            let (addr, rid3, secret3, game, ticket) = (address.clone(), rid.clone(), secret.clone(), profile.path.clone(), guest.clone());
            let claimed = blocking(move || {
                let peer = crate::identity::peer_identity()?;
                let mut old: Vec<String> = Vec::new();
                if let Some(j) = wf::worlds(&app).get(&rid3) {
                    old.extend(j.old.iter().cloned());
                    old.push(j.user.clone());
                }
                if let Some(t) = wf::config_ticket(&game) {
                    old.push(t);
                }
                old.retain(|u| wf::ticket_ok(u));
                old.dedup();
                wf::claim_at(&addr, &peer, &rid3, &secret3, ticket.as_ref().map(|(u, p)| (u.as_str(), p.as_str())), &old)
            })
            .await;
            match (claimed, guest) {
                (Ok(up), _) => up,
                (Err(_), Some(g)) => g,
                (Err(e), None) => return Err(format!("Your friend's world: {e}")),
            }
        } else {
            guest.ok_or("That invite has no login in it.")?
        };
        if lasting {
            let j = wf::Joined { relay: relay.clone(), secret: secret.clone(), lan: lan_kept, user: user.clone(), old: Vec::new(), name: name.clone(), at: now_secs() };
            let _ = wf::remember(&settings::app_dir(), &rid, j);
        }
        let _ = crate::herokey::set_login(&profile.path, &user, None);
        let s = crate::hybrid::Session { user: user.clone(), pass: pass.clone(), realm: None, enter: false, pick: None, make: None };
        let (message, hybrid) = point_and_play_with(&core, &client, &address, true, Some(s), true)?;
        let (auto, why) = match hybrid {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        };
        let guest = GuestJoin { auto, account: user, password: if auto { String::new() } else { pass }, why, message };
        return Ok(Joined { address, name, guest: Some(guest), way: way.into(), tried });
    }
    let (address, name) = blocking(move || realms::knock(&format!("https://{relay}"), &rid, &secret)).await?;
    if let Some(k) = core.kit() {
        let own_den = crate::host::invite(&k).as_deref() == Some(link.as_str())
            && crate::host::inspect(Some(&k), None).realm.is_some_and(|r| r.profile == crate::host::profiles::DEN);
        if own_den {
            if let Ok(mut g) = DEN_ADDRESS.lock() {
                *g = Some(address.clone());
            }
        }
    }
    point_and_play(&core, &client, &address, false)?;
    Ok(Joined { address, name, guest: None, way: String::new(), tried: Vec::new() })
}

/// What Invite friends made: the link to send (one friend, 24 hours) and the tunnel as it is now.
#[derive(Serialize)]
struct DenInvite {
    link: String,
    /// The same way in as a code to paste (DEN-...: host/denhost.rs), the relay way.
    code: String,
    name: String,
    expires: u64,
    tunnel: crate::host::litenet::TunnelState,
    /// The ways in the link, in the order the friend's launcher tries them (host/denways.rs).
    ways: Vec<InviteWay>,
}

/// One way in: "lan" (same network), "own" (your own address), "relay"; on or not, and why, in a line.
#[derive(Serialize, Debug, Clone, PartialEq)]
struct InviteWay {
    id: &'static str,
    ok: bool,
    line: String,
}

/// The same network way: Den Lite's direct listener opened on this ask (`<dens>/den-lite.direct-want`; it reads it
/// within 2 seconds) and what it says. Some("ip:port") when open.
fn open_lan(dens: &Path) -> (Option<String>, InviteWay) {
    use crate::host::denways;
    let off = |line: String| (None, InviteWay { id: "lan", ok: false, line });
    let Some(ip) = denways::lan_address() else { return off("Same network: this PC isn't on a home or office network.".into()) };
    let want = dens.join(denways::WANT_FILE);
    if std::fs::write(&want, ip.to_string()).is_err() {
        return off("Same network: couldn't ask your Den's server.".into());
    }
    for _ in 0..25 {
        if !want.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    if want.exists() {
        let _ = std::fs::remove_file(&want);
        return off("Same network: your Den's server is from before this way (update Leyline, then Play again).".into());
    }
    match std::fs::read_to_string(dens.join(denways::DIRECT_FILE)).map_err(|e| e.to_string()).and_then(|t| denways::direct_state(&t)) {
        Ok((ip, lp, _)) => (Some(format!("{ip}:{lp}")), InviteWay { id: "lan", ok: true, line: format!("Same network: on, at {ip} (friends at your home or on your network come in here first).") }),
        Err(why) => off(format!("Same network: off ({why}). Allow Den Lite in your firewall for private networks, then Invite again.")),
    }
}

/// Invite friends, Den Lite (owner 1 Nov: "invite a friend through the Leyline network and host locally
/// instantly"): Den Lite started if it isn't, its tunnel to the relay up (host/litenet.rs: private, the relay only
/// passes the game's connections through), a login made for this one friend, and the link that carries it.
#[tauri::command]
async fn den_invite(name: Option<String>) -> Result<DenInvite, String> {
    blocking(move || den_invite_now(name.as_deref())).await
}

/// Invite friends, now (the button, and "Invite a friend" in the game's Den panel through Den Lite's ask).
fn den_invite_now(name: Option<&str>) -> Result<DenInvite, String> {
    den_invite_with(name, true)
}

/// `tunnel`: false for the owner's den-test: the same login and link, but the relay tunnel isn't started (the
/// test's friend comes in on this PC, through Den Lite's friends' listener).
fn den_invite_with(name: Option<&str>, tunnel: bool) -> Result<DenInvite, String> {
    use crate::host::{lite::{self, DenLite}, litenet};
    let tunnel_on = tunnel;
    {
        let app = crate::settings::app_dir();
        let mut l = lite::Binary::find(&app);
        if !l.available() {
            // one step for the host (owner 5 Nov: "invite a friend easy 1 step"): Your Den's server is got here, as
            // Play solo gets it (one small download, its sha256 checked)
            if !tunnel_on {
                return Err("Press Play solo once first: it gets your Den's server.".to_string());
            }
            lite::install(&app).map_err(|e| format!("{e} Check this PC is online, then press Invite a friend again."))?;
            l = lite::Binary::find(&app);
        }
        if !l.status().running {
            l.start()?;
            for _ in 0..50 {
                if litenet::remote_ports(&l.den_dir()).is_some() && l.status().running {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
        if litenet::remote_ports(&l.den_dir()).is_none() {
            return Err("Your Den's server is from before friends could join: update Leyline, then try again.".into());
        }
        let root = lite::root(&app);
        let seed = litenet::load_or_create_key(&litenet::key_path(&root))?;
        let secret = lite::invite_secret(&root)?;
        let tunnel = if tunnel {
            // kept on for the next start of Leyline (host/denhost.rs: den_share_resume)
            crate::host::denhost::set_sharing(&root, true);
            den_tunnel(&l, &root)?
        } else {
            let public = litenet::public_key(&seed)?;
            litenet::TunnelState { relay: realms::RELAY.into(), realm_id: litenet::realm_id(&public), ..Default::default() }
        };
        let g = l.guest_add(name)?;
        let link = litenet::invite_link(realms::RELAY, &tunnel.realm_id, &secret, Some((&g.user, &g.pass)));
        // the other ways (host/denways.rs): the same network, the player's own address
        let dens = l.den_dir();
        let (lan, lan_way) = open_lan(&dens);
        let own = std::fs::read_to_string(dens.join(crate::host::denways::HOST_FILE)).ok().map(|t| t.trim().to_string()).filter(|h| crate::host::denways::host_ok(h));
        let own_way = match (&own, lan.is_some()) {
            (None, _) => InviteWay { id: "own", ok: false, line: "Your own address: not set (only for a router you set up, or your own server; see the guide).".into() },
            (Some(h), true) => InviteWay { id: "own", ok: true, line: format!("Your own address: {h} (your router must send ports 3724 and 8085 to this PC).") },
            (Some(h), false) => InviteWay { id: "own", ok: false, line: format!("Your own address: {h}, but your Den's direct way is off (see Same network).") },
        };
        let at = own.filter(|_| lan.is_some());
        let relay_way = if !tunnel_on {
            InviteWay { id: "relay", ok: false, line: "Leyline relay: not started (a test).".into() }
        } else if tunnel.last_error.is_empty() {
            InviteWay { id: "relay", ok: true, line: "Leyline relay: on (works from anywhere; the relay only passes the game's connection through).".into() }
        } else {
            InviteWay { id: "relay", ok: false, line: format!("Leyline relay: {}", tunnel.last_error) }
        };
        let code = crate::host::denhost::code_from_link(&link).unwrap_or_default();
        let link = crate::host::denways::with_ways(&link, lan.as_deref(), at.as_deref());
        Ok(DenInvite { link, code, name: g.name, expires: g.expires, tunnel, ways: vec![lan_way, own_way, relay_way] })
    }
}

/// The relay tunnel for this Den Lite (host/litenet.rs), as Invite friends and the resume start it.
fn den_tunnel(l: &crate::host::lite::Binary, root: &Path) -> Result<crate::host::litenet::TunnelState, String> {
    use crate::host::{lite::DenLite, litenet};
    let seed = litenet::load_or_create_key(&litenet::key_path(root))?;
    let secret = crate::host::lite::invite_secret(root)?;
    Ok(litenet::start(litenet::Setup {
        endpoint: litenet::Endpoint::relay(realms::RELAY),
        relay_name: realms::RELAY.into(),
        seed,
        name: "Den Lite".into(),
        invite_secret: secret,
        den_dir: l.den_dir(),
        realm_list: false,
    }))
}

/// Sharing kept across restarts (owner 4 Nov: "it keeps working across restarts"): when Leyline starts and the host
/// was sharing (host/denhost.rs `sharing`), Den Lite starts (no game) and the tunnel comes back, so friends' links
/// work after the host's PC restarts. Stop sharing ends it.
fn den_share_resume() -> Result<bool, String> {
    use crate::host::{denhost, lite::{self, DenLite}, litenet};
    let app = crate::settings::app_dir();
    let root = lite::root(&app);
    if !denhost::sharing(&root) {
        return Ok(false);
    }
    let l = lite::Binary::find(&app);
    if !l.available() {
        return Ok(false);
    }
    if !l.status().running {
        l.start()?;
        for _ in 0..50 {
            if litenet::remote_ports(&l.den_dir()).is_some() && l.status().running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    if litenet::remote_ports(&l.den_dir()).is_none() {
        return Err("Your Den's server is from before friends could join.".into());
    }
    den_tunnel(&l, &root).map(|_| true)
}

/// The Friends list on Your Den (host/denhost.rs): who is in now, the friends' logins that still work, and whether
/// the Den is shared.
#[derive(Serialize)]
struct DenFriends {
    running: bool,
    sharing: bool,
    online: crate::host::denhost::Online,
    invites: Vec<crate::host::lite::GuestInfo>,
    tunnel: crate::host::litenet::TunnelState,
}

#[tauri::command]
async fn den_friends() -> Result<DenFriends, String> {
    blocking(|| {
        use crate::host::{denhost, lite::{self, DenLite}};
        let app = crate::settings::app_dir();
        let l = lite::Binary::find(&app);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let running = l.available() && l.status().running;
        let mut online = denhost::read_online(&l.den_dir(), now);
        if !running {
            online = denhost::Online::default();
        }
        let invites = if l.exe.is_some() { l.guest_list().unwrap_or_default() } else { Vec::new() };
        Ok(DenFriends { running, sharing: denhost::sharing(&lite::root(&app)), online, invites, tunnel: crate::host::litenet::state() })
    })
    .await
}

/// One thing the host does for a friend from the Friends list: `kick` (out now, their link stops), `build` (on or
/// off; `who` "all" for every friend), `keep` (their link works a week from their last visit). The line to show.
#[tauri::command]
async fn den_friend(action: String, who: String, on: bool) -> Result<String, String> {
    blocking(move || {
        use crate::host::{denhost, lite::{self, DenLite}};
        let line = denhost::ask_line(&action, &who, on).ok_or("That isn't something Your Den can do for a friend.")?;
        let l = lite::Binary::find(&crate::settings::app_dir());
        if l.exe.is_none() {
            return Err("Press Play solo once first: it gets your Den's server.".into());
        }
        if l.status().running {
            denhost::ask(&l.den_dir(), &line)?;
            if let Some(said) = denhost::answer(&l.den_dir(), std::time::Duration::from_millis(2500)) {
                return Ok(said);
            }
            // an older Den Lite doesn't read asks: what the file can do, it does
            let _ = std::fs::remove_file(l.den_dir().join(denhost::HOST_ASK_FILE));
        }
        match action.as_str() {
            "kick" => l.guest_revoke(&who).map(|_| "Their link no longer works.".to_string()),
            "build" if who == "all" => Err("Start your Den first (Play solo): every friend's switch is for a Den that is running.".into()),
            _ => l.guest_set(&who, &action, on).map(|_| match (action.as_str(), on) {
                ("build", true) => "They can build when they come in.".to_string(),
                ("build", false) => "They can look around, not build.".to_string(),
                (_, true) => "Kept: their link works for a week from their last visit.".to_string(),
                _ => "No longer kept.".to_string(),
            }),
        }
    })
    .await
}

/// "Your own address" for friends (host/denways.rs): a host name or IP (optionally `:port`) that reaches this PC
/// from the internet, kept in `<dens>/den-lite.direct-host` for Den Lite; empty takes it away. What is set now.
#[tauri::command]
fn den_own_address(host: Option<String>) -> Result<String, String> {
    use crate::host::{denways, lite::DenLite};
    let dens = crate::host::lite::Binary::find(&crate::settings::app_dir()).den_dir();
    let f = dens.join(denways::HOST_FILE);
    match host.map(|h| h.trim().to_string()) {
        None => {}
        Some(h) if h.is_empty() => {
            let _ = std::fs::remove_file(&f);
        }
        Some(h) if denways::host_ok(&h) => {
            std::fs::create_dir_all(&dens).map_err(|e| e.to_string())?;
            std::fs::write(&f, &h).map_err(|e| format!("Couldn't keep your address: {e}"))?;
        }
        Some(_) => return Err("That isn't an address: a name like den.example.org or an IP, optionally :port.".into()),
    }
    Ok(std::fs::read_to_string(&f).map(|t| t.trim().to_string()).unwrap_or_default())
}

/// My models (host/denmodels.rs): a dropped file, its bytes as base64 (the UI reads them from the drop).
#[derive(serde::Deserialize)]
struct DroppedFile {
    name: String,
    data: String,
}

fn den_models_dir() -> PathBuf {
    use crate::host::lite::DenLite;
    crate::host::lite::Binary::find(&crate::settings::app_dir()).den_dir()
}

/// The player's own models in Your Den.
#[tauri::command]
fn den_models() -> Vec<crate::host::denmodels::Model> {
    crate::host::denmodels::list(&den_models_dir())
}

/// A drop of model files added (checked; nothing uploaded). It is in the Den the next time it starts.
#[tauri::command]
async fn den_models_add(files: Vec<DroppedFile>, label: Option<String>) -> Result<crate::host::denmodels::Model, String> {
    blocking(move || {
        if files.len() > crate::host::denfiles::MAX_FILES {
            return Err(format!("Up to {} files at once.", crate::host::denfiles::MAX_FILES));
        }
        let mut got = Vec::new();
        for f in files {
            if f.data.len() > crate::host::denfiles::MAX_TOTAL / 3 * 4 + 8 {
                return Err(format!("File {} is too big.", f.name));
            }
            let b = leyline_terrain::b64::decode(&f.data).ok_or_else(|| format!("File {} didn't arrive whole.", f.name))?;
            got.push((f.name, b));
        }
        let dir = den_models_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        crate::host::denmodels::add(&dir, &got, label.as_deref())
    })
    .await
}

#[tauri::command]
fn den_models_remove(slug: String) -> Result<bool, String> {
    crate::host::denmodels::remove(&den_models_dir(), &slug)
}

/// The tunnel now (Your Den's Friends line).
#[tauri::command]
fn den_net_state() -> crate::host::litenet::TunnelState {
    crate::host::litenet::state()
}

/// Stop sharing: the tunnel down and every friend's login taken back.
#[tauri::command]
async fn den_net_stop() -> Result<(), String> {
    blocking(|| {
        crate::host::litenet::stop();
        crate::host::denhost::set_sharing(&crate::host::lite::root(&crate::settings::app_dir()), false);
        let l = crate::host::lite::Binary::find(&crate::settings::app_dir());
        if l.exe.is_some() {
            l.guest_revoke_all()?;
        }
        Ok(())
    })
    .await
}

/// Ready for friends? (host/ready.rs): the checklist the host sees before sending an invite, for Your Den on Den
/// Lite ("lite") or the Host pack realm ("docker"), and the same as text to paste (no invite secret, no logins).
#[derive(Serialize)]
struct HostReady {
    #[serde(flatten)]
    ready: crate::host::ready::Ready,
    diagnostics: String,
}

#[tauri::command]
async fn host_ready(core: State<'_, Core>, kind: Option<String>) -> Result<HostReady, String> {
    let kit = core.kit();
    blocking(move || {
        let r = host_ready_now(kind.as_deref().unwrap_or("lite"), kit);
        let diagnostics = crate::host::ready::diagnostics(&r, env!("CARGO_PKG_VERSION"));
        Ok(HostReady { ready: r, diagnostics })
    })
    .await
}

fn host_ready_now(kind: &str, kit: Option<PathBuf>) -> crate::host::ready::Ready {
    use crate::host::{denways, lite::{self, DenLite}, litenet, ready};
    let t = Duration::from_secs(6);
    let mut f = ready::Facts { kind: kind.to_string(), os: std::env::consts::OS.into(), relay: realms::RELAY.into(), ..Default::default() };
    let mut secret: Option<String> = None;
    if kind == "classic" {
        // Leyline Solo's first-era world (classic_invite): its login on this PC, the tunnel, the same network
        let p = crate::host::classic::Paths::new(&settings::app_dir());
        let login = ready::login_answers(&format!("127.0.0.1:{}", crate::host::classic::LOGIN_PORT), t);
        f.running = login.is_ok() && crate::host::classic::status(&p).running;
        f.login = Some(login);
        let st = litenet::state();
        f.tunnel = Some((st.running, st.connected, st.last_error.clone()));
        if litenet::key_path(&p.share).is_file() {
            if let Ok(pk) = litenet::load_or_create_key(&litenet::key_path(&p.share)).and_then(|k| litenet::public_key(&k)) {
                f.realm_id = litenet::realm_id(&pk);
            }
            secret = crate::host::worldshare::invite_secret(&p.share).ok();
        }
        f.lan = Some(crate::host::worldshare::lan_layout() == Some(crate::host::worldshare::Layout::Classic));
    } else if let Some(k) = crate::host::engine::Kind::from_word(kind).filter(|k| *k != crate::host::engine::Kind::BossRush) {
        // a single player world on the Built-in engine (world_invite): its login on this PC, the tunnel, the same network
        let p = crate::host::engine::Paths::for_kind(&settings::app_dir(), k);
        let login = ready::login_answers(crate::host::engine::REALM, t);
        f.running = login.is_ok() && crate::host::engine::other_running(&p).is_none();
        f.login = Some(login);
        let st = litenet::state();
        f.tunnel = Some((st.running, st.connected, st.last_error.clone()));
        let share = crate::host::worldshare::dir(&p.root);
        if let Ok(pk) = litenet::load_or_create_key(&litenet::key_path(&share)).and_then(|k| litenet::public_key(&k)) {
            f.realm_id = litenet::realm_id(&pk);
        }
        secret = crate::host::worldshare::invite_secret(&share).ok();
        f.lan = Some(crate::host::worldshare::lan_state().is_some());
    } else if kind == "docker" {
        if let Some(link) = kit.as_deref().and_then(crate::host::invite) {
            if let Some((relay, rid, s)) = realms::parse_invite(&link) {
                (f.relay, f.realm_id, secret) = (relay, rid, Some(s));
            } else if let Some((relay, rid)) = ready::link_parts(&link) {
                (f.relay, f.realm_id) = (relay, rid);
            }
        }
        let login = ready::login_answers("127.0.0.1:3724", t);
        f.running = login.is_ok();
        f.login = Some(login);
    } else {
        let app = settings::app_dir();
        let l = lite::Binary::find(&app);
        f.running = l.available() && l.status().running;
        if let (true, Some((lp, _))) = (f.running, litenet::remote_ports(&l.den_dir())) {
            f.login = Some(ready::login_answers(&format!("127.0.0.1:{lp}"), t));
        }
        // a tunnel just started gets a few seconds to come up
        let mut st = litenet::state();
        for _ in 0..40 {
            if !st.running || st.connected {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
            st = litenet::state();
        }
        f.tunnel = Some((st.running, st.connected, st.last_error.clone()));
        f.realm_id = st.realm_id.clone();
        let root = lite::root(&app);
        if f.realm_id.is_empty() {
            if let Ok(p) = litenet::load_or_create_key(&litenet::key_path(&root)).and_then(|k| litenet::public_key(&k)) {
                f.realm_id = litenet::realm_id(&p);
            }
        }
        secret = lite::invite_secret(&root).ok();
        f.lan = std::fs::read_to_string(l.den_dir().join(denways::DIRECT_FILE)).ok().map(|t| denways::direct_state(&t).is_ok());
    }
    f.relay_ms = ready::connect_ms(&f.relay, 443, 3, Duration::from_secs(4));
    if !f.realm_id.is_empty() {
        let base = format!("https://{}", f.relay);
        match net::get_json(&format!("{base}/api/realms/{}", f.realm_id), t) {
            Ok(v) => {
                let online = v.get("online").and_then(serde_json::Value::as_bool).unwrap_or(false);
                f.relay_online = Some(Ok(online));
                if online {
                    // the way a friend's game comes in: the invite's knock (a private world), then the world's own
                    // login port at the relay
                    let at = match &secret {
                        Some(s) => realms::knock(&base, &f.realm_id, s).map(|(a, _)| a),
                        None => match (v.get("address").and_then(serde_json::Value::as_str), v.get("auth_port").and_then(serde_json::Value::as_u64)) {
                            (Some(a), Some(p)) => Ok(format!("{a}:{p}")),
                            _ => Err("the relay gave no address for your world".to_string()),
                        },
                    };
                    f.path = Some(at.and_then(|a| ready::login_answers(&ready::with_port(&a), t)));
                }
            }
            Err(e) => f.relay_online = Some(Err(e)),
        }
    }
    f.sleep_min = ready::sleep_now();
    ready::judge(&f)
}

/// The friend's side, after Join: does the host's world answer at the address Join used? Ok(ms), or why not (a world
/// that isn't answering is what loops the game between Connecting and Connected).
#[tauri::command]
async fn realm_answers(address: String) -> Result<u32, String> {
    let address = crate::host::ready::with_port(address.trim());
    if !crate::host::ready::may_test(&address, &[crate::addresses::RELAY_HOST, crate::addresses::OLD_RELAY_HOST]) {
        return Err("Leyline only tests its own relay and your own network.".into());
    }
    blocking(move || crate::host::ready::login_answers(&address, Duration::from_secs(6))).await
}

/// A fix button on the checklist that opens the PC's own settings: "power" (sleep) or "firewall" (allowed apps).
#[tauri::command]
fn host_ready_fix(app: AppHandle, what: String) -> Result<String, String> {
    if !cfg!(windows) {
        return Err(match what.as_str() {
            "power" => "Open your computer's power or energy settings and turn off sleep while friends play.".into(),
            _ => "Open your computer's firewall settings and allow Den Lite for your home network.".into(),
        });
    }
    match what.as_str() {
        "power" => {
            app.opener().open_url("ms-settings:powersleep", None::<&str>).map_err(|e| e.to_string())?;
            Ok("Your power settings are open: set Sleep to Never (plugged in), then press Check again.".into())
        }
        "firewall" => {
            std::process::Command::new("control").arg("firewall.cpl").spawn().map_err(|e| e.to_string())?;
            Ok("Your firewall settings are open: Allow an app, then tick Den Lite for Private networks.".into())
        }
        _ => Err("That isn't a fix Leyline knows.".into()),
    }
}

/// What Play solo did: the line to show, and whether the game signs in by itself (`auto`) or the login screen
/// wants the local password typed (`why` says why not, in plain words).
#[derive(Serialize, Debug, PartialEq)]
struct SoloPlay {
    message: String,
    auto: bool,
    why: String,
    account: String,
    password: String,
}

/// The local Den's sign-in for one start (crate::hybrid): Den Lite's local password under our login screen code's own
/// account name (crate::denclick::GLUE_ACCOUNT, so Den Lite's log shows which way the game came in), its realm
/// picked by name, and straight into the world (a character made first when there is none).
fn den_session() -> crate::hybrid::Session {
    use crate::host::lite;
    crate::hybrid::Session { user: crate::denclick::GLUE_ACCOUNT.into(), pass: lite::PASSWORD.into(), realm: Some(lite::REALM.into()), enter: true, pick: None, make: None }
}

/// Den Lite on this PC, started for this game folder when it isn't running, its login port answering (up to 15 s).
/// Got on first use (one small download from its release, checked against its sha256). True: it was already running.
fn den_lite_up(game: &Path) -> Result<bool, String> {
    use crate::host::lite::{self, DenLite};
    let app = crate::settings::app_dir();
    let mut l = lite::Binary::find(&app);
    if !l.available() {
        lite::install(&app)?; // first use: the release's program, its sha256 checked (host/lite.rs)
        l = lite::Binary::find(&app);
    } else if !l.status().running && lite::swap_in_next(&app) {
        // a newer Den Lite downloaded in the background since the last start: in place now, before it starts
        l = lite::Binary::find(&app);
    }
    // the release asked for a newer one (at most every 12 hours, in the background): it is used at the next start
    lite::update_in_background(&app);
    let l = l.with_client(game);
    let was = l.status().running;
    // a local world on this PC holds the same login port: said by name (never stopped unasked)
    if !was {
        if let Some(k) = running_world() {
            return Err(format!("{} is running on this PC and uses the same login port as Your Den. Stop it on the Single player page (everyone is saved), then press Play again.", k.title()));
        }
    }
    l.start()?;
    for _ in 0..50 {
        if l.status().running {
            return Ok(was);
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    Err("Den Lite started but isn't answering yet. Try again in a moment.".to_string())
}

/// The process id of the game the last start began (point_and_play_with).
static LAST_GAME_PID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// One My Den start the launcher signs in for (crate::denclick).
struct DenStart {
    pid: u32,
    glue: bool,
    game: PathBuf,
    profile: ClientProfile,
    wine: crate::settings::Wine,
}

/// My Den's start, after Den Lite is up: its account and realm in Config.wtf (never the password), the handoff for
/// our login screen code (one start, local only) unless this game refused it before, the client pointed at
/// 127.0.0.1 and started. Returns what the page shows and the start for crate::denclick to drive.
fn den_launch(core: &Core, client: &str) -> Result<(SoloPlay, DenStart), String> {
    use crate::host::lite;
    let profile = core.client(client)?;
    let _ = crate::herokey::set_login(&profile.path, lite::ACCOUNT, Some(lite::REALM));
    let refused = crate::hybrid::refused(&profile.path);
    let session = if refused { None } else { Some(den_session()) };
    let (message, hybrid) = point_and_play_with(core, client, lite::ADDRESS, true, session, true)?;
    let glue = hybrid.is_ok();
    let st = DenStart { pid: LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed), glue, game: profile.path.clone(), profile, wine: core.snapshot().wine };
    let keys = cfg!(windows);
    let why = match (&hybrid, refused) {
        (Ok(()), _) => String::new(),
        (Err(_), true) => "This game doesn't take Leyline's login screen, so the launcher signs you in.".into(),
        (Err(e), false) => e.clone(),
    };
    let mut play = solo_result(message, if glue || keys { Ok(()) } else { Err(why.clone()) });
    if !glue && keys {
        play.why = why; // said in the log; the page shows nothing to type
    }
    Ok((play, st))
}

/// After a My Den start: signs in (crate::denclick::drive) unless our login screen code did, answers the game's
/// asks (Invite a friend, Shape the ground) while it runs, and takes the handoff out when it closes. A game that
/// refused our login screen code is started once more without it. `test`: the den-test step's hold (seconds).
fn den_watch(st: DenStart, test: Option<u64>, cover: Option<Arc<crate::cover::Signals>>) -> crate::denclick::Outcome {
    use crate::denclick::{self, Driver, Outcome, Process};
    use crate::host::lite;
    let app = crate::settings::app_dir();
    let root = lite::root(&app);
    let den_dir = crate::host::dens::dir(&root);
    let mut st = st;
    let game_path = st.game.clone();
    let testing = test.is_some();
    let test_root = root.clone();
    let mut asks = move || {
        let Some((id, what)) = denclick::take_ask(&den_dir_for_asks()) else { return };
        let d = den_dir_for_asks();
        match what.as_str() {
            "invite" => match den_invite_with(None, !testing) {
                Ok(i) => {
                    if testing {
                        // for the den-test only, which reads it once and removes it (never into a log)
                        let _ = crate::settings::write_atomic(&test_root.join(denclick::INVITE_FILE), serde_json::json!({ "link": i.link }).to_string().as_bytes());
                    }
                    denclick::answer(&d, id, Some(&i.link), None)
                }
                Err(e) => denclick::answer(&d, id, None, Some(&format!("No invite link: {e}"))),
            },
            _ if testing => {
                let r = forge_test(&app, &game_path, &crate::p2p::cmds::den_root());
                let line = if r["ok"] == true {
                    "Test: Forge's files for your Den are ready (Forge isn't opened during a test).".to_string()
                } else {
                    format!("Test: Forge's files couldn't be made: {}", r["error"].as_str().unwrap_or("?"))
                };
                let _ = crate::settings::write_atomic(&test_root.join(denclick::FORGE_REPORT), r.to_string().as_bytes());
                denclick::answer(&d, id, None, Some(&line));
            }
            _ => {
                let r = crate::host::forge::open(&app, &game_path, &crate::p2p::cmds::den_root(), None);
                let line = match r {
                    Ok(_) => "Forge is open: shape your Den's ground there. Close it when you're done; restart your game to walk on it.".to_string(),
                    Err(e) => format!("Forge didn't open: {e}"),
                };
                denclick::answer(&d, id, None, Some(&line));
            }
        }
    };
    let start = denclick::now_ms();
    let mut out = denclick::drive(&mut Process(st.pid), &den_dir, &root, Driver::new(start, st.glue, test.is_some()), lite::PASSWORD, st.pid, &mut asks);
    // the sign-in cover goes now: in the world, or the sign-in stopped (a refused login screen code: the new start
    // below has no cover, the player sees the game)
    if let Some(c) = &cover {
        c.finish(out == Outcome::InWorld);
    }
    if out == Outcome::GlueRefused {
        // the game closed on our login screen code: never again for this game, and once more without it
        // recorded with when and why: it expires after a launcher update, a week, or Repair (hybrid::refused)
        let _ = crate::hybrid::write_refused(&st.game, "The game closed on Leyline's login screen code; the launcher's keys sign in instead.");
        let _ = crate::hybrid::remove(&st.game);
        if let Ok(plan) = launch::plan(&st.profile, &st.wine, launch::Os::current()) {
            if let Ok(child) = launch::run(&plan) {
                st.pid = child.id();
                st.glue = false;
                let start = denclick::now_ms();
                out = denclick::drive(&mut Process(st.pid), &den_dir, &root, Driver::new(start, false, test.is_some()), lite::PASSWORD, st.pid, &mut asks);
                std::mem::forget(child); // the game runs on; it isn't the launcher's to stop
            }
        }
    }
    let mut game = Process(st.pid);
    use crate::denclick::Game;
    match test {
        // den-test: wait for the step's screenshot (den-click.shot), then close the game
        Some(hold) => {
            let shot = denclick::shot_marker(&root);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(hold);
            while std::time::Instant::now() < until && game.alive() && !shot.exists() {
                asks();
                // the test's fallback when its Den Lite doesn't take commands from the Dens folder: a typed line
                if let Some((id, line)) = denclick::take_type(&root) {
                    let ok = denclick::type_chat(st.pid, &line);
                    let _ = crate::settings::write_atomic(&root.join(denclick::TYPED_FILE), serde_json::json!({ "id": id, "ok": ok }).to_string().as_bytes());
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            let _ = std::fs::remove_file(&shot);
            close_game(st.pid);
        }
        None => {
            while game.alive() {
                asks();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
    // the handoff out as soon as the game lets go of the archive
    for _ in 0..40 {
        if crate::hybrid::scrub(&st.game).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    out
}

/// The den-test's "Shape the ground": what Open in Forge makes before it starts Forge (host/terrain.rs `workspace`:
/// the Den's tile, its ground file in Forge's form, the one tile Forge may save, the newest Den file to draw), made
/// and checked, but Forge itself is never started by a test.
fn forge_test(app: &Path, game: &Path, den_root: &Path) -> serde_json::Value {
    use crate::host::{forge, terrain};
    let st = forge::state(app);
    match terrain::workspace(app, game, den_root) {
        Ok((ws, d)) => {
            let ground = ws.dir.join(format!("{}.terrain.json", terrain::GROUND));
            let tile = std::fs::metadata(&ws.adt).map(|m| m.len()).unwrap_or(0);
            let args = terrain::forge_args(game, &ws, &d);
            serde_json::json!({
                "ok": tile > 0 && ground.is_file() && ws.locked.is_file(), "installed": st.installed, "opened": false,
                "workspace": ws.dir.display().to_string(), "tile_bytes": tile, "ground": ground.is_file(), "locked": ws.locked.is_file(),
                "den_file": ws.den_file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()),
                "map": d.tile.map, "tile": format!("{},{}", d.tile.x, d.tile.y), "args": args.len(),
            })
        }
        Err(e) => serde_json::json!({ "ok": false, "installed": st.installed, "opened": false, "error": e }),
    }
}

fn den_dir_for_asks() -> PathBuf {
    crate::host::dens::dir(&crate::host::lite::root(&crate::settings::app_dir()))
}

/// Close the game the test started (it is the test's own start).
fn close_game(pid: u32) {
    if pid == 0 {
        return;
    }
    let mut c = if cfg!(windows) {
        let mut c = std::process::Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/F"]);
        c
    } else {
        let mut c = std::process::Command::new("kill");
        c.arg(pid.to_string());
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let _ = c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// Play solo: Your Den on Den Lite (host/lite.rs), one click into the world. Den Lite started for this game folder
/// (gets it on first use), then [`den_launch`] starts the game and [`den_watch`] signs in by itself: with our login
/// screen code where the game takes it, else with the launcher's keys (crate::denclick), straight into the Den.
#[tauri::command]
async fn den_play_lite(app: AppHandle, core: State<'_, Core>, client: String) -> Result<SoloPlay, String> {
    let profile = core.client(&client)?;
    if profile.build != crate::clients::Build::V335a {
        return Err("Your Den plays with a different game folder: pick or add it in the play bar, where its version shows.".into());
    }
    addons_before_launch(&core, &client, false).await;
    let game = profile.path.clone();
    blocking(move || den_lite_up(&game)).await?;
    let (play, st) = den_launch(&core, &client)?;
    let cover = if core.snapshot().signin_cover {
        cover_start(&app, st.pid, crate::cover::Mode::Den, "", Some((crate::host::dens::dir(&crate::host::lite::root(&settings::app_dir())), crate::denclick::now_ms())))
    } else {
        None
    };
    std::thread::spawn(move || den_watch(st, None, cover));
    Ok(play)
}

/// `Leyline den-play [--test] [--client <id>] [--hold <s>]` (crate::denclick::parse_cli): My Den from the command
/// line, with no window of the launcher's own: the owner's den-test step (tools/den-check). Writes
/// `<den-lite root>/den-click.json` / `den-click.log`; exits 0 when the game reached the world.
pub fn den_play_cli(cli: crate::denclick::Cli) -> i32 {
    use crate::host::lite::{self, DenLite};
    std::hint::black_box(crate::denclick::CLI_MARK.as_ptr());
    std::hint::black_box(crate::denclick::TEST_MARK.as_ptr());
    let dir = settings::app_dir();
    let core = Core {
        settings: Mutex::new(settings::load(&dir)),
        dir,
        known_urls: Mutex::new(HashSet::new()),
        mods_lock: Arc::new(Mutex::new(())),
        games: Mutex::new(Vec::new()),
        upd: Mutex::new(UpdState::default()),
        deep_link: Mutex::new(crate::deeplink::Inbox::default()),
    };
    let root = lite::root(&settings::app_dir());
    let fail = |why: String| {
        let r = crate::denclick::Report { phase: "done".into(), outcome: format!("Stopped({why:?})"), start: crate::denclick::now_ms(), ..Default::default() };
        crate::denclick::write_report(&root, &r);
        1
    };
    let s = core.snapshot();
    let pick = cli.client.clone().or(s.selected_client.clone()).filter(|id| s.client(id).is_some_and(|c| c.build == clients::Build::V335a))
        .or_else(|| s.clients.iter().find(|c| c.build == clients::Build::V335a).map(|c| c.id.clone()));
    let Some(client) = pick else { return fail("no game folder for Your Den in the launcher".into()) };
    let game = match core.client(&client) {
        Ok(p) => p.path,
        Err(e) => return fail(e),
    };
    // Leyline's own addons up to date first, as the launcher window does when it starts (the Den bar is in them;
    // 2 Nov: the den-test's game still had the addons from before the Den bar)
    let mut addons_note = String::new();
    let app_dir = settings::app_dir();
    if crate::addons::prefs(&app_dir).auto_update {
        if let (Some(d), Ok(p)) = (crate::addons::available(&app_dir, crate::addons::DESCRIPTOR_URL).pack, core.client(&client)) {
            for (_, r) in crate::addons::auto_update(&[p], &d, &app_dir) {
                addons_note = match r {
                    Ok(done) => format!("Leyline addons updated to {}", done.version),
                    Err(e) => format!("Leyline addons not updated: {e}"),
                };
            }
        }
    }
    // the test's Den Lite takes its commands through the Dens folder (labs/den-lite testctl.rs): one already running
    // doesn't, so it is restarted for the test (and stopped after it, as one the test started)
    let mut restarted = false;
    if cli.test {
        lite::TEST_CONTROL.store(true, std::sync::atomic::Ordering::Relaxed);
        let l = lite::Binary::find(&settings::app_dir());
        if l.status().running {
            let _ = l.stop();
            restarted = true;
        }
    }
    let was = match den_lite_up(&game) {
        Ok(w) => w,
        Err(e) => return fail(e),
    };
    let (launch_msg, st) = match den_launch(&core, &client) {
        Ok((p, st)) => (p.message, st),
        Err(e) => return fail(e),
    };
    // the Den's ground (host/terrain.rs patch-T: the game's own grass) while the game runs: for the test's report
    let ground = crate::host::terrain::slot(&game).map(|(_, ours)| ours).unwrap_or(false);
    let out = den_watch(st, cli.test.then_some(cli.hold), None);
    if cli.test && !was {
        let _ = lite::Binary::find(&settings::app_dir()).stop();
    }
    // the report's last word: closed
    if let Ok(t) = std::fs::read_to_string(root.join(crate::denclick::REPORT)) {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&t) {
            v["closed"] = true.into();
            v["den_lite_stopped"] = (cli.test && !was).into();
            v["den_lite_restarted"] = restarted.into();
            v["launch"] = launch_msg.clone().into();
            v["addons"] = addons_note.clone().into();
            v["ground"] = ground.into();
            // what the Den's patch-T holds: our white texture, the island's models gone, the sky and the zone's name
            v["ground_detail"] = serde_json::to_value(crate::host::terrain::last_status(&root)).unwrap_or_default();
            let _ = settings::write_atomic(&root.join(crate::denclick::REPORT), v.to_string().as_bytes());
        }
    }
    if out == crate::denclick::Outcome::InWorld { 0 } else { 1 }
}

/// `Leyline play-test --official [--game <folder>] [--world] [--hold <s>]` (crate::playtest): PLAY on the official realm
/// from the command line with no launcher window, as the PLAY button does (play_official), with the launcher's own
/// sign-in and game folders: the owner's client-smoke step (tools/client-smoke, owner 2 Nov 23:33: "a robust smoke
/// test with screenshots each phase ... access to my client for testing and debug"). Writes `<data>/play-test.json`
/// (phases, the game's process id for its screenshots, whether the login screen got the sign-in; never the password),
/// keeps the game open `--hold` seconds, closes it, takes the one-time login out of the archive. Exit 0: started with
/// the sign-in handed to the game.
pub fn play_test_cli(cli: crate::playtest::Cli) -> i32 {
    std::hint::black_box(crate::playtest::CLI_MARK.as_ptr());
    std::hint::black_box(crate::playtest::CHECKS_MARK.as_ptr());
    let dir = settings::app_dir();
    let report_path = dir.join(crate::playtest::REPORT);
    let mut rep = serde_json::json!({ "mark": crate::playtest::CLI_MARK, "phase": "start", "started_ms": crate::denclick::now_ms() });
    let save = |rep: &serde_json::Value| {
        let _ = settings::write_atomic(&report_path, serde_json::to_string_pretty(rep).unwrap_or_default().as_bytes());
    };
    let fail = |rep: &mut serde_json::Value, why: String| {
        rep["phase"] = "done".into();
        rep["error"] = why.into();
        let _ = settings::write_atomic(&dir.join(crate::playtest::REPORT), serde_json::to_string_pretty(rep).unwrap_or_default().as_bytes());
        1
    };
    // --fresh (main.rs pointed the data folder at an empty one): nothing of a launcher already here may be in it
    let had_settings = dir.join(settings::FILE_NAME).exists();
    rep["fresh"] = serde_json::json!({ "asked": cli.fresh, "empty": !had_settings });
    if cli.fresh && had_settings {
        return fail(&mut rep, "--fresh needs an empty data folder: this one already has a launcher's settings".into());
    }
    let core = Core {
        settings: Mutex::new(settings::load(&dir)),
        dir: dir.clone(),
        known_urls: Mutex::new(HashSet::new()),
        mods_lock: Arc::new(Mutex::new(())),
        games: Mutex::new(Vec::new()),
        upd: Mutex::new(UpdState::default()),
        deep_link: Mutex::new(crate::deeplink::Inbox::default()),
    };
    let same = |a: &std::path::Path, b: &std::path::Path| a.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches(['\\', '/']));
    // --game a folder the launcher doesn't have yet (a fresh data folder has none): added as Find your game files does
    if let Some(g) = &cli.game {
        if !core.snapshot().clients.iter().any(|c| same(&c.path, g)) {
            let added = clients::detect(g).map_err(|e| e.to_string()).and_then(|det| {
                core.update(|s| {
                    let profile = clients::new_profile(g, &det, &s.clients);
                    s.clients.push(profile.clone());
                    if s.selected_client.is_none() {
                        s.selected_client = Some(profile.id.clone());
                    }
                    Ok(serde_json::json!({ "build": profile.build.version(), "complete": det.complete, "missing": det.missing }))
                })
            });
            match added {
                Ok(a) => rep["added"] = a,
                Err(e) => return fail(&mut rep, format!("couldn't add the game folder: {e}")),
            }
        }
    }
    // --game more than once: the other folders are added too (a player with several versions); PLAY uses the first
    if !cli.more.is_empty() {
        let mut more = Vec::new();
        for g in &cli.more {
            if core.snapshot().clients.iter().any(|c| same(&c.path, g)) {
                continue;
            }
            let r = clients::detect(g).map_err(|e| e.to_string()).and_then(|det| {
                core.update(|s| {
                    let profile = clients::new_profile(g, &det, &s.clients);
                    s.clients.push(profile.clone());
                    Ok(serde_json::json!({ "ok": true, "build": profile.build.version(), "name": profile.name, "complete": det.complete }))
                })
            });
            more.push(r.unwrap_or_else(|e| serde_json::json!({ "ok": false, "why": e })));
        }
        rep["more_added"] = more.into();
    }
    // --checks: what the game folder box does with each thing a player can drop on it (a fresh profile: Settings ›
    // Testing off, so a magnet or a .torrent gets the one sentence); the probes' small files removed after
    let probe_dir = dir.join(crate::playtest::PROBE_DIR);
    if cli.checks {
        let torrents = crate::clienttorrent::load_testing(&dir).client_torrents;
        rep["checks"] = serde_json::json!({ "mark": crate::playtest::CHECKS_MARK, "torrent_switch": torrents });
        rep["checks"]["drops"] = match crate::playtest::drop_probes(&probe_dir, cli.game.as_deref()) {
            Ok(p) => crate::playtest::run_drops(&p, torrents),
            Err(e) => serde_json::json!([{ "name": "the drop probes", "ok": false, "got": e.to_string() }]),
        };
        save(&rep);
    }
    // the test Hero Key (the first-run tool's test account), only from the environment: signed in, then forgotten.
    // --checks: three ways, as a player does them: the key file found in a folder (the sign-in screen's Downloads
    // look, the same reading a dropped file gets), log out, then the key pasted with the junk a paste brings
    if let Some(key) = std::env::var_os(crate::playtest::KEY_ENV) {
        std::env::remove_var(crate::playtest::KEY_ENV);
        let key = key.to_string_lossy().to_string();
        let out = if cli.checks {
            let file = probe_dir.join("Leyline Hero Key.txt");
            let by_file = std::fs::create_dir_all(&probe_dir)
                .and_then(|_| std::fs::write(&file, format!("Leyline Hero Key\r\nKey: {key}\r\n")))
                .map_err(|e| e.to_string())
                .and_then(|_| crate::social::find_key_file(&probe_dir).ok_or_else(|| "the key file wasn't found".to_string()))
                .and_then(|f| crate::social::key_file_text(std::path::Path::new(&f.path)))
                .and_then(|t| crate::social::sign_in_key(&dir, &t));
            let _ = std::fs::remove_file(&file);
            let file_ok = by_file.is_ok() && crate::social::load_token(&dir).is_some();
            crate::social::sign_out(&dir);
            let out_ok = crate::social::load_token(&dir).is_none();
            let by_paste = crate::social::sign_in_key(&dir, &format!("  Key: {key} \n"));
            rep["checks"]["signin"] = serde_json::json!({
                "file": file_ok, "file_why": by_file.as_ref().err().cloned().unwrap_or_default(),
                "logout": out_ok,
                "paste": by_paste.is_ok(), "paste_why": by_paste.as_ref().err().cloned().unwrap_or_default()
            });
            by_paste
        } else {
            crate::social::sign_in_key(&dir, &key)
        };
        drop(key);
        match out {
            Ok(v) => {
                let me = v.get("me").cloned().unwrap_or_default();
                rep["key_signin"] = serde_json::json!({ "ok": true, "name": me.get("name").cloned().unwrap_or_default() });
            }
            Err(e) => return fail(&mut rep, format!("the test Hero Key didn't sign in: {e}")),
        }
    }
    let s = core.snapshot();
    let pick = match &cli.game {
        Some(g) => s.clients.iter().find(|c| same(&c.path, g)).map(|c| c.id.clone()),
        None => s.selected_client.clone().filter(|id| s.client(id).is_some_and(|c| c.build == clients::Build::V335a))
            .or_else(|| s.clients.iter().find(|c| c.build == clients::Build::V335a).map(|c| c.id.clone())),
    };
    let Some(client) = pick else { return fail(&mut rep, "no 3.3.5a game folder in the launcher (or not the one --game names)".into()) };
    let profile = match core.client(&client) {
        Ok(p) => p,
        Err(e) => return fail(&mut rep, e),
    };
    rep["client"] = serde_json::json!({ "id": client, "name": profile.name, "build": profile.build.version(), "path": profile.path });
    rep["signed_in"] = crate::social::load_token(&dir).is_some().into();
    rep["phase"] = "login".into();
    save(&rep);
    let login = crate::herokey::official_login(&dir);
    rep["website_login"] = match &login {
        Ok(l) => serde_json::json!({ "ok": true, "account": l.user }),
        Err(e) => serde_json::json!({ "ok": false, "why": e }),
    };
    let session = login.as_ref().ok().map(|l| {
        let _ = crate::herokey::set_account_name(&profile.path, &l.user);
        crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: cli.world, pick: None, make: None }
    });
    let pass = login.as_ref().ok().map(|l| l.pass.clone());
    drop(login);
    rep["glue"] = serde_json::to_value(crate::hybrid::status(&profile.path)).unwrap_or_default();
    rep["phase"] = "starting".into();
    save(&rep);
    let (message, hybrid) = match point_and_play_with(&core, &client, OFFICIAL_REALM, true, session, false) {
        Ok(x) => x,
        Err(e) => return fail(&mut rep, e),
    };
    let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
    rep["message"] = message.into();
    rep["hybrid"] = match &hybrid {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "why": e }),
    };
    rep["pid"] = pid.into();
    // as PLAY does: no Forever UI, the launcher's keys (keys_sign_in)
    let keys = match (&hybrid, pass) {
        (Err(_), Some(pass)) if pid != 0 => Some(keys_sign_in(pid, pass, None)),
        _ => None,
    };
    rep["keys"] = if keys.is_some() { serde_json::json!({ "started": true }) } else { serde_json::Value::Null };
    rep["phase"] = "running".into();
    rep["running_ms"] = crate::denclick::now_ms().into();
    save(&rep);
    // the game stays open for the screenshots, then it is closed (unless it closed itself)
    let until = std::time::Instant::now() + std::time::Duration::from_secs(cli.hold.min(crate::playtest::HOLD_MAX));
    let mut closed_itself = false;
    while std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let gone = core.games.lock().map(|mut g| g.iter_mut().all(|c| c.try_wait().ok().flatten().is_some())).unwrap_or(false);
        if gone {
            closed_itself = true;
            break;
        }
    }
    if let Ok(mut g) = core.games.lock() {
        for c in g.iter_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    let mut scrubbed = false;
    for _ in 0..40 {
        if crate::hybrid::scrub(&profile.path).is_ok() {
            scrubbed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if let Some(k) = keys {
        rep["keys"] = match k.join() {
            Ok(Ok(())) => serde_json::json!({ "started": true, "typed": true }),
            Ok(Err(e)) => serde_json::json!({ "started": true, "typed": false, "why": e }),
            Err(_) => serde_json::json!({ "started": true, "typed": false, "why": "the keys thread stopped" }),
        };
    }
    rep["closed_itself"] = closed_itself.into();
    rep["session_left"] = crate::hybrid::has_session(&profile.path).into();
    rep["scrubbed"] = scrubbed.into();
    // --checks, with the game closed: a community realm and back, then loadouts Clean and back
    if cli.checks {
        rep["phase"] = "checks".into();
        save(&rep);
        rep["checks"]["realm_switch"] = realm_switch_check(&core, &client);
        rep["checks"]["loadouts"] = crate::playtest::loadout_round_trip(&profile.path);
        let _ = std::fs::remove_dir_all(&probe_dir);
    }
    rep["phase"] = "done".into();
    rep["done_ms"] = crate::denclick::now_ms().into();
    save(&rep);
    let typed = rep["keys"]["typed"].as_bool() == Some(true);
    if (hybrid.is_ok() || typed) && !closed_itself { 0 } else { 1 }
}

/// play-test --checks: the game folder pointed at a community realm from the list (as Play on a community realm does,
/// without starting the game), then back at the official realm. `ok` null when the list has none for this folder.
fn realm_switch_check(core: &Core, client: &str) -> serde_json::Value {
    let list = realms::fetch_all(&core.dir);
    let build = core.client(client).ok().map(|p| p.build);
    let host = |a: &str| a.trim().to_ascii_lowercase().split(':').next().unwrap_or("").to_string();
    let Some(r) = list.realms.iter().find(|r| {
        !r.official
            && r.address.as_deref().is_some_and(|a| clients::valid_realm_address(a) && !crate::addresses::is_official_realm(a))
            && (r.build.is_none() || r.build == build)
    }) else {
        return serde_json::json!({ "ok": serde_json::Value::Null, "why": "no community realm for this game folder on the list right now", "listed": list.realms.len() });
    };
    let addr = r.address.clone().unwrap_or_default();
    let to = point_and_play(core, client, &addr, false);
    let at = core.client(client).ok().and_then(|p| clients::read_realmlist(&p)).unwrap_or_default();
    let there = to.is_ok() && host(&at) == host(&addr);
    let back = point_and_play(core, client, OFFICIAL_REALM, false);
    let home_at = core.client(client).ok().and_then(|p| clients::read_realmlist(&p)).unwrap_or_default();
    let home = back.is_ok() && crate::addresses::is_official_realm(&home_at);
    serde_json::json!({
        "ok": there && home, "realm": r.name, "there": there, "back": home,
        "why": to.err().or(back.err()).unwrap_or_default()
    })
}

fn solo_result(message: String, hybrid: Result<(), String>) -> SoloPlay {
    use crate::host::lite;
    let (auto, why) = match hybrid {
        Ok(()) => (true, String::new()),
        Err(e) => (false, e),
    };
    SoloPlay { message, auto, why, account: lite::ACCOUNT.into(), password: if auto { String::new() } else { lite::PASSWORD.into() } }
}

/// Your Den on the Docker Den: its account (the realm's GM name) filled in on the client's login screen, so PLAY
/// needs only the password the player chose on the join page.
#[tauri::command]
fn den_prefill_account(core: State<Core>, client: String) -> Result<String, String> {
    let profile = core.client(&client)?;
    let kit = core.kit().ok_or("Get the Host pack first.")?;
    let acct = crate::host::yourden::account(&kit);
    if acct.is_empty() {
        return Ok(String::new());
    }
    crate::herokey::set_account_name(&profile.path, &acct).map_err(|e| format!("Couldn't write Config.wtf: {e}"))?;
    Ok(acct)
}

/// "Make my account": the Docker Den's own invite link (from the Host pack, never from the page) opened in the browser,
/// where the owner makes the account they named, as friends do.
#[tauri::command]
fn den_join_page(app: AppHandle, core: State<Core>) -> Result<(), String> {
    let kit = core.kit().ok_or("Get the Host pack first.")?;
    let link = crate::host::invite(&kit).ok_or("Your Den has no invite link yet: start it once first.")?;
    realms::parse_invite(&link).ok_or("Your Den's invite link doesn't look right: make a new one under Realm tools.")?;
    app.opener().open_url(link, None::<&str>).map_err(|e| e.to_string())
}

// ---- Single player's Built-in engine (host/engine.rs, docs/BUILTIN-ENGINE.md): no Docker. The page (single.js)
// follows engine_progress as it follows host_progress for the Docker engine.

fn engine_paths() -> crate::host::engine::Paths {
    crate::host::engine::Paths::new(&settings::app_dir())
}

/// Installed, set up, running (each port), the player's login once made.
#[tauri::command]
async fn engine_status() -> Result<crate::host::engine::EngineStatus, String> {
    blocking(|| Ok(crate::host::engine::status(&engine_paths()))).await
}

/// Set Single player up on the Built-in engine (whatever isn't done yet: the engine, the maps from this game folder,
/// the database, the account) and start it. `update`: take the release's newer engine first.
#[tauri::command]
fn engine_setup(core: State<Core>, client: String, update: Option<bool>) -> Result<String, String> {
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Single player plays with a different game folder: pick one of its version.".into());
    }
    crate::host::engine::begin(&settings::app_dir(), Some(profile.path), update.unwrap_or(false))
}

/// Start it again (set up already).
#[tauri::command]
fn engine_start() -> Result<String, String> {
    crate::host::engine::begin(&settings::app_dir(), None, false)
}

/// The progress line of whichever world is setting up or starting: Single player's engine, or Leyline Solo's
/// (`kind: "classic"`, the same as classic_progress) while its job runs or when it was the last one.
#[tauri::command]
fn engine_progress() -> crate::host::engine::Job {
    let e = crate::host::engine::progress();
    let c = crate::host::classic::progress();
    if c.running || (!e.running && e.idle && !c.idle) {
        c
    } else {
        e
    }
}

/// Everyone saved, everything stopped.
#[tauri::command]
async fn engine_stop() -> Result<String, String> {
    blocking(|| {
        world_share_off(crate::host::engine::Kind::Single);
        crate::host::engine::stop(&engine_paths())
    })
    .await
}

#[tauri::command]
async fn engine_logs(lines: Option<usize>) -> Result<String, String> {
    blocking(move || Ok(crate::host::engine::logs(&engine_paths(), lines.unwrap_or(200).clamp(20, 2000)))).await
}

/// A back-up of the saves (accounts, characters, the bots): the zip's path.
#[tauri::command]
async fn engine_backup() -> Result<String, String> {
    blocking(|| crate::host::engine::backup(&engine_paths()).map(|p| p.display().to_string())).await
}

/// The release looked at for a newer engine; the status after.
#[tauri::command]
async fn engine_update_check() -> Result<crate::host::engine::EngineStatus, String> {
    blocking(|| {
        let p = engine_paths();
        crate::host::engine::check_update(&p)?;
        Ok(crate::host::engine::status(&p))
    })
    .await
}

#[derive(Serialize)]
struct EnginePlay {
    message: String,
    account: String,
    password: String,
}

/// Play Single player on the Built-in engine: the account filled in on the login screen and the realm picked after
/// it (Config.wtf), the client pointed at 127.0.0.1:3724, then started.
#[tauri::command]
fn engine_play(app: AppHandle, core: State<Core>, client: String) -> Result<EnginePlay, String> {
    use crate::host::engine;
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Single player plays with a different game folder: pick one of its version.".into());
    }
    let _ = profile;
    // a sync command: the Hero Key's login from the id kept last time (no network here)
    let hero = crate::host::oneclick::hero_identity_cached(&core.dir);
    let r = world_play_now(&core, &client, engine::Kind::Single, None, None, false, hero, Some(&app))?;
    Ok(EnginePlay { message: r.message, account: r.account, password: r.password })
}

// ---- Single player's worlds by kind (host/engine.rs KINDS; docs/SINGLE-PLAYER-ONE-CLICK.md): the original world in
// one click, Quick start, the Characters page (On this PC / Online), Invite a friend from a world. The page
// (single.js, characters.js) names a world by its word: "single", "vanilla", "bossrush".

fn world_kind(kind: &str) -> Result<crate::host::engine::Kind, String> {
    crate::host::engine::Kind::from_word(kind).ok_or_else(|| "[other] That isn't one of this PC's worlds.".to_string())
}

fn world_paths(k: crate::host::engine::Kind) -> crate::host::engine::Paths {
    crate::host::engine::Paths::for_kind(&settings::app_dir(), k)
}

#[tauri::command]
async fn world_status(kind: String) -> Result<crate::host::engine::EngineStatus, String> {
    let k = world_kind(&kind)?;
    blocking(move || {
        let st = crate::host::engine::status(&world_paths(k));
        if st.running {
            ingame_sync_start();
        }
        Ok(st)
    })
    .await
}

/// Set a world up (whatever isn't done: the engine, the maps from this game folder, the database, the login) and
/// start it; its progress is engine_progress (the line's `kind` says whose).
/// Set a world up (see above). `client` "" : the game folder this world's setup was started with (a setup that goes
/// on after a restart), else the one `world_find_folder` would pick. Refused with the disk check's `[disk]` message
/// when the drive is short of space. Errors start with their code (docs/SINGLE-PLAYER-ONE-CLICK.md §contract).
#[tauri::command]
async fn world_setup(core: State<'_, Core>, client: String, kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    let profile = world_client(&core, k, &client)?;
    // the first setup puts the Leyline addons in too (companions, the bot bar): the world plays with them
    addons_before_launch(&core, &profile.id, true).await;
    blocking(move || world_setup_with(k, &profile)).await
}

/// world_setup once its game folder is known (blocking: the disk check may ask the release for its sizes); the
/// single player playtest (single_play_test_cli) calls this too.
fn world_setup_with(k: crate::host::engine::Kind, profile: &ClientProfile) -> Result<String, String> {
    use crate::host::{engine as eng, oneclick::tag};
    let p = world_paths(k);
    let set_up = eng::set_up(&eng::have(&p, &eng::load_state(&p), false));
    if !set_up {
        let pf = eng::preflight(&p);
        if !pf.ok {
            return Err(pf.message);
        }
    }
    eng::begin_setup(&settings::app_dir(), k, Some(profile.path.clone()), false, Some(&profile.id)).map_err(|e| tag("other", e))
}

/// A world's settings (host/spsettings.rs): companions, dungeon buddies, roaming bots, XP rate; realm.env over the
/// defaults, `chosen` once the player picked them, and whether it is set up / running.
#[tauri::command]
async fn world_settings_get(kind: String) -> Result<crate::host::spsettings::WorldSettings, String> {
    use crate::host::{engine as eng, spsettings as sps};
    let k = world_kind(&kind)?;
    if !sps::supports(k) {
        return Err(tag_other(format!("{} has no world settings.", k.title())));
    }
    blocking(move || {
        let p = world_paths(k);
        let st = eng::status(&p);
        let mut w = sps::current(&p, &eng::load_state(&p));
        w.set_up = st.set_up;
        w.running = st.running;
        Ok(w)
    })
    .await
}

/// The mods in a single player world (host/worldmods.rs): the engine's own modules as its server gets them at its next
/// start, read from disk (no network). The Single player page's "Mods in this world" lists them beside the game folder's.
#[tauri::command]
async fn world_mods_get(kind: String) -> Result<crate::host::worldmods::WorldMods, String> {
    use crate::host::{engine as eng, worldmods as wm};
    let k = world_kind(&kind)?;
    if !wm::supports(k) {
        return Err(tag_other(format!("{} has no world mods.", k.title())));
    }
    blocking(move || {
        let p = world_paths(k);
        let st = eng::status(&p);
        Ok(wm::WorldMods { kind: k.word().into(), mods: wm::list(&p), set_up: st.set_up, running: st.running })
    })
    .await
}

/// Turn one of a world's optional modules off or on (its realm.env); a running world takes it at its next start
/// (`restart`: the page offers Restart now).
#[tauri::command]
async fn world_mods_set(kind: String, id: String, on: bool) -> Result<crate::host::worldmods::Saved, String> {
    use crate::host::{engine as eng, worldmods as wm};
    let k = world_kind(&kind)?;
    blocking(move || {
        let p = world_paths(k);
        let running = eng::status(&p).running;
        wm::set(&p, &id, on, running).map_err(tag_other)
    })
    .await
}

/// Change a world's settings (only the ones given), written into its realm.env; the world needn't be set up yet. A
/// running world takes them at its next start (`restart`: the page offers Restart).
/// The page sends the keys as written here (companions_auto, ...), not in camelCase.
#[tauri::command(rename_all = "snake_case")]
async fn world_settings_set(
    kind: String,
    companions_auto: Option<bool>,
    companions_follow: Option<u8>,
    dungeon_auto: Option<bool>,
    roaming_bots: Option<u32>,
    xp_rate: Option<f64>,
) -> Result<crate::host::spsettings::Saved, String> {
    use crate::host::{engine as eng, spsettings as sps};
    let k = world_kind(&kind)?;
    let c = sps::Change { companions_auto, companions_follow, dungeon_auto, roaming_bots, xp_rate };
    blocking(move || {
        let p = world_paths(k);
        let running = eng::status(&p).running;
        let mut r = sps::set(&p, &c, running).map_err(tag_other)?;
        r.settings.set_up = eng::status(&p).set_up;
        Ok(r)
    })
    .await
}

fn tag_other(e: impl Into<String>) -> String {
    let e: String = e.into();
    if e.starts_with('[') { e } else { crate::host::oneclick::tag("other", e) }
}

#[tauri::command]
fn world_start(kind: String) -> Result<String, String> {
    let r = crate::host::engine::begin_kind(&settings::app_dir(), world_kind(&kind)?, None, false);
    ingame_sync_start();
    r
}

/// The game folder a world plays with: `client` when given (it must be of the world's version), else the one its
/// setup was started with, else the only fitting one. `[folder]` when there's none.
fn world_client(core: &Core, k: crate::host::engine::Kind, client: &str) -> Result<ClientProfile, String> {
    use crate::host::oneclick::tag;
    let snap = core.snapshot();
    let fits = |c: &&ClientProfile| c.build == world_build(k);
    if !client.is_empty() {
        let c = snap.client(client).ok_or_else(|| tag("folder", "Pick your game folder first."))?;
        if !fits(&c) {
            return Err(tag("folder", format!("{} plays with a different game folder: pick one of its version.", k.title())));
        }
        return Ok(c.clone());
    }
    let saved = crate::host::engine::load_state(&world_paths(k)).setup_client;
    if let Some(c) = snap.client(&saved).filter(fits) {
        return Ok(c.clone());
    }
    let all: Vec<&ClientProfile> = snap.clients.iter().filter(fits).collect();
    match all.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(tag("folder", format!("Add your game folder first: {} reads its maps from it once.", k.title()))),
        _ => Err(tag("folder", "Pick which game folder to play with.")),
    }
}

/// The game folder's version every local world plays with (the Wrath client; docs/SINGLE-PLAYER-ONE-CLICK.md).
fn world_build(_k: crate::host::engine::Kind) -> clients::Build {
    clients::Build::V335a
}

#[derive(Serialize)]
struct FoundFolder {
    id: String,
    name: String,
    path: String,
    build: clients::Build,
    /// Added to the game folders just now (found on this PC).
    added: bool,
}

#[derive(Serialize)]
struct FolderFind {
    /// The game folder to use: exactly one fits.
    picked: Option<String>,
    found: Vec<FoundFolder>,
}

/// Find the game folder by itself: the game folders already added that fit the world; none: the PC's usual places
/// are looked through (oneclick::scan_games) and each fitting, complete folder found is added as a game folder (the
/// same as adding it by hand). Exactly one fitting: `picked`.
#[tauri::command]
async fn world_find_folder(core: State<'_, Core>, kind: String) -> Result<FolderFind, String> {
    let k = world_kind(&kind)?;
    let want = world_build(k);
    let mut found = folders_known(&core, want);
    if found.is_empty() {
        let known: Vec<PathBuf> = core.snapshot().clients.iter().map(|c| c.path.clone()).collect();
        let hits = blocking(move || Ok(folders_scan(&known, want))).await?;
        found = folders_add(&core, hits);
    }
    let picked = (found.len() == 1).then(|| found[0].id.clone());
    Ok(FolderFind { picked, found })
}

fn found_row(c: &ClientProfile, added: bool) -> FoundFolder {
    FoundFolder { id: c.id.clone(), name: c.name.clone(), path: c.path.display().to_string(), build: c.build, added }
}

/// world_find_folder's first look: the game folders already added that fit.
fn folders_known(core: &Core, want: clients::Build) -> Vec<FoundFolder> {
    core.snapshot().clients.iter().filter(|c| c.build == want && c.path.is_dir()).map(|c| found_row(c, false)).collect()
}

/// world_find_folder's second: the PC's usual places looked through (blocking) for complete folders of `want`.
fn folders_scan(known: &[PathBuf], want: clients::Build) -> Vec<(PathBuf, Detection)> {
    use crate::host::oneclick as oc;
    oc::scan_games(&oc::usual_roots(), 3, 20_000)
        .into_iter()
        .filter(|p| !known.contains(p))
        .filter_map(|p| clients::detect(&p).ok().filter(|d| d.build == want && d.complete).map(|d| (p, d)))
        .collect()
}

/// Each folder found added as a game folder, exactly as adding one by hand.
fn folders_add(core: &Core, hits: Vec<(PathBuf, Detection)>) -> Vec<FoundFolder> {
    hits.into_iter().filter_map(|(path, det)| add_profile(core, path, det).ok().map(|a| found_row(&a.profile, true))).collect()
}

/// world_find_folder without the async runtime (the single player playtest).
fn find_folder_now(core: &Core, k: crate::host::engine::Kind) -> FolderFind {
    let want = world_build(k);
    let mut found = folders_known(core, want);
    if found.is_empty() {
        let known: Vec<PathBuf> = core.snapshot().clients.iter().map(|c| c.path.clone()).collect();
        found = folders_add(core, folders_scan(&known, want));
    }
    let picked = (found.len() == 1).then(|| found[0].id.clone());
    FolderFind { picked, found }
}

/// The disk check before setup: `{ok, free_gb, need_gb, message}` (message: the `[disk]` error, empty when it fits).
#[tauri::command]
async fn world_preflight(core: State<'_, Core>, kind: String, client: String) -> Result<crate::host::oneclick::Preflight, String> {
    let k = world_kind(&kind)?;
    world_client(&core, k, &client)?;
    blocking(move || Ok(crate::host::engine::preflight(&world_paths(k)))).await
}

/// A world whose setup stopped midway (the launcher closed): its word, for the page to offer or go on
/// (`world_setup` with client "" goes on with the same game folder).
#[tauri::command]
fn world_resume_pending() -> Option<String> {
    crate::host::oneclick::resume_pending(&settings::app_dir()).map(|k| k.word().to_string())
}

/// Repair a world (a background job on engine_progress, as setup): every engine file checked and only the broken
/// ones fetched again, the maps read again only if they're gone, the login set up again, the ports checked; the
/// sign-in in this game folder gets another chance. The saves are never touched.
#[tauri::command]
fn world_repair(core: State<Core>, kind: String, client: String) -> Result<String, String> {
    world_repair_now(&core, world_kind(&kind)?, &client)
}

fn world_repair_now(core: &Core, k: crate::host::engine::Kind, client: &str) -> Result<String, String> {
    let profile = world_client(core, k, client)?;
    crate::hybrid::clear_refused(&profile.path);
    crate::host::engine::begin_repair(&settings::app_dir(), k, Some(profile.path))
}

/// A local world whose servers run on this PC now (the one this launcher started first).
fn running_world() -> Option<crate::host::engine::Kind> {
    use crate::host::engine as eng;
    let order = eng::active().into_iter().chain(eng::KINDS);
    order.into_iter().find(|k| eng::status(&world_paths(*k)).running)
}

/// The page answered "close Leyline with my world running" (or there was nothing to ask): the next close goes through.
static CLOSE_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Leyline's main window is closing (Settings › world_stop_on_close, on by default): with a world running and no
/// friend in it, the window goes and the world is saved and stopped, then Leyline exits; with friends in it, the
/// page asks first (`leyline:close-world`, answered by close_leyline). True: the close waits (prevented).
fn close_with_world(window: &tauri::Window) -> bool {
    use crate::host::engine as eng;
    if CLOSE_ASKED.load(std::sync::atomic::Ordering::SeqCst) || !settings::load(&settings::app_dir()).world_stop_on_close {
        return false;
    }
    let Some(k) = running_world() else { return false };
    let friends = eng::friends_online(&world_paths(k)).unwrap_or(0);
    if friends > 0 {
        use tauri::Emitter;
        let _ = window.emit("leyline:close-world", serde_json::json!({ "kind": k.word(), "title": k.title(), "friends": friends }));
        return true;
    }
    let _ = window.hide();
    close_stopping(window.app_handle().clone(), Some(k));
    true
}

/// Save and stop `k` (sharing ends with it), tell friends Leyline is offline, then exit.
fn close_stopping(app: AppHandle, k: Option<crate::host::engine::Kind>) {
    use crate::host::engine as eng;
    CLOSE_ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
        if let Some(k) = k {
            let p = world_paths(k);
            eng::setup_log(&p, "Leyline is closing: saving and stopping the world");
            world_share_off(k);
            let _ = eng::stop(&p);
        }
        if let Some(h) = crate::p2p::friendnet::current() {
            crate::p2p::friendnet::stop(&h);
        }
        app.exit(0);
    });
}

/// The page's answer to `leyline:close-world`: `stop` saves and stops the world, then closes Leyline; false closes
/// Leyline and leaves the world running (friends who came through the launcher are cut off).
#[tauri::command]
fn close_leyline(app: AppHandle, stop: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
    close_stopping(app, if stop { running_world() } else { None });
}

/// Settings › Stop my world when Leyline closes.
#[tauri::command]
fn set_world_stop_on_close(core: State<Core>, on: bool) -> Result<(), String> {
    core.update(|s| {
        s.world_stop_on_close = on;
        Ok(())
    })
}

/// Settings › Stop my world when the game closes.
#[tauri::command]
fn set_world_stop_on_exit(core: State<Core>, on: bool) -> Result<(), String> {
    core.update(|s| {
        s.world_stop_on_exit = on;
        Ok(())
    })
}

/// Everyone saved, everything stopped; sharing it with friends ends with it.
#[tauri::command]
async fn world_stop(kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || {
        world_share_off(k);
        crate::host::engine::stop(&world_paths(k))
    })
    .await
}

#[tauri::command]
async fn world_logs(kind: String, lines: Option<usize>) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || Ok(crate::host::engine::logs(&world_paths(k), lines.unwrap_or(200).clamp(20, 2000)))).await
}

#[tauri::command]
async fn world_backup(kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || crate::host::engine::backup(&world_paths(k)).map(|p| p.display().to_string())).await
}

/// "Move my characters here": every one of the player's characters in a world onto the login Play uses now (the
/// signed-in Hero Key's, else the world's own), inside a back-up taken first. The world must be running. How many.
#[tauri::command]
async fn world_gather_chars(core: State<'_, Core>, kind: String) -> Result<String, String> {
    use crate::host::{engine as eng, oneclick::tag};
    let k = world_kind(&kind)?;
    if k == eng::Kind::BossRush {
        return Err(tag("other", "Boss Rush's heroes stay on its own login."));
    }
    let dir = core.dir.clone();
    blocking(move || {
        let p = world_paths(k);
        if !eng::status(&p).running {
            return Err(tag("other", format!("Start {} first (press Play), then move your characters.", k.title())));
        }
        let to = match crate::host::oneclick::hero_identity(&dir) {
            Some(h) => {
                eng::hero_login(&p, &h)?;
                h
            }
            None => eng::load_state(&p).account,
        };
        let n = eng::gather_chars(&p, &to)?;
        Ok(if n == 0 { "Your characters are all on this login already.".to_string() } else { format!("{n} characters moved: they're on the login Play uses now. A back-up was taken first.") })
    })
    .await
}

/// The world's own login password, when the player asks for it (Show, Copy on the Manage card): never in the status
/// the pages poll.
#[tauri::command]
async fn world_password(kind: String) -> Result<String, String> {
    use crate::host::engine as eng;
    let k = world_kind(&kind)?;
    blocking(move || {
        let s = eng::load_state(&world_paths(k));
        if !s.account_made {
            return Err(format!("Set {} up first.", k.title()));
        }
        Ok(s.password)
    })
    .await
}

/// A world's back-ups, newest first (when, why), for its Restore list.
#[tauri::command]
async fn world_backups(kind: String) -> Result<Vec<crate::host::engine::BackupInfo>, String> {
    let k = world_kind(&kind)?;
    blocking(move || Ok(crate::host::engine::backups_info(&world_paths(k)))).await
}

/// Put one of a world's back-ups back (its name from world_backups): the world as it is now backed up first, the
/// world stopped (sharing with friends ends with it), the back-up loaded; Play starts it again.
#[tauri::command]
async fn world_restore(kind: String, name: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || {
        world_share_off(k);
        crate::host::engine::restore(&world_paths(k), &name)
    })
    .await
}

/// Ask the release for a newer world engine (the engine every world shares) when the last look is over 12 hours old
/// (`force`: now), waiting 5 s at most; the world's status after (its `update`).
#[tauri::command]
async fn world_update_check(kind: String, force: Option<bool>) -> Result<crate::host::engine::EngineStatus, String> {
    use crate::host::engine as eng;
    let k = world_kind(&kind)?;
    blocking(move || {
        let p = world_paths(k);
        eng::check_update_cached(&p, if force.unwrap_or(false) { 0 } else { eng::UPDATE_CHECK_EVERY });
        Ok(eng::status(&p))
    })
    .await
}

/// Update a world's engine (a background job on engine_progress, as setup): its saves backed up first, the new
/// engine put in place, the world started again. The maps, saves and settings stay.
#[tauri::command]
fn world_update(kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    crate::host::engine::begin_update(&settings::app_dir(), k)
}

#[derive(Serialize)]
struct WorldChoices {
    races: Vec<crate::host::localworld::RaceChoice>,
    /// The original world: the first era's races and classes only.
    first_era: bool,
    /// A free name to suggest.
    name: String,
}

/// Quick start's choices for a world.
#[tauri::command]
fn world_choices(kind: String) -> Result<WorldChoices, String> {
    use crate::host::localworld as lw;
    let k = world_kind(&kind)?;
    if k == crate::host::engine::Kind::BossRush {
        return Err("Boss Rush's heroes are made on its own page.".into());
    }
    let first_era = k == crate::host::engine::Kind::Vanilla;
    let taken: Vec<String> = lw::read_kept(&crate::host::engine::chars_file(&world_paths(k))).chars.into_iter().map(|c| c.name).collect();
    Ok(WorldChoices { races: lw::choices(first_era), first_era, name: lw::name_for("", &taken).unwrap_or_default() })
}

/// Quick start: a new level-1 character made on the game's own create screen, then entered.
#[derive(Deserialize, Debug, Clone)]
struct QuickPick {
    name: Option<String>,
    race: u8,
    class: u8,
    female: Option<bool>,
}

#[derive(Serialize)]
struct WorldPlay {
    message: String,
    kind: String,
    /// Signed in by itself (our login screen code); else the login to type once.
    auto: bool,
    account: String,
    password: String,
    /// Quick start's character's name.
    made: String,
    /// Played on the Hero Key's own login in this world (signed in); false: the world's own login.
    hero: bool,
    /// Played on another of the player's logins here, the one that holds the characters (signed out, or another Hero
    /// Key signed in): the page offers "Move my characters here" (world_gather_chars).
    elsewhere: bool,
}

/// Play a world that is running: the game signed in by itself (the hybrid login, as Your Den and Boss Rush do: the
/// world's own login, made by the launcher with a generated password, never one the player typed), the world's
/// realm picked; `pick` enters that character, `quick` makes a new one first and enters it; neither: character select.
/// `first`: the first entry from one click: no characters on this login yet, Quick start with `quick` (or a random
/// pick when none is given) and straight into the world; with characters, the last one played. With neither `pick`,
/// `quick` nor `first`: the last character played when it's known, else character select.
#[tauri::command]
async fn world_play(app: AppHandle, core: State<'_, Core>, client: String, kind: String, pick: Option<String>, quick: Option<QuickPick>, first: Option<bool>) -> Result<WorldPlay, String> {
    let k = world_kind(&kind)?;
    // signed in: the Hero Key's own login in this world (asks the website who; the id kept last time when offline)
    let hero = if k == crate::host::engine::Kind::BossRush {
        None
    } else {
        let dir = core.dir.clone();
        blocking(move || Ok(crate::host::oneclick::hero_identity(&dir))).await?
    };
    world_play_now(&core, &client, k, pick, quick, first.unwrap_or(false), hero, Some(&app))
}

/// Each Play of a world counts one up: a game closing stops its world only when no Play came after it.
static WORLD_PLAYS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[allow(clippy::too_many_arguments)]
/// `app`: the launcher window's app, for the sign-in cover over the game's login screen (crate::cover, Mode::Single;
/// none from the command line).
fn world_play_now(core: &Core, client: &str, k: crate::host::engine::Kind, pick: Option<String>, quick: Option<QuickPick>, first: bool, hero: Option<String>, app: Option<&AppHandle>) -> Result<WorldPlay, String> {
    use crate::host::{bossrush as br, engine as eng, localworld as lw, oneclick::tag};
    let profile = world_client(core, k, client)?;
    let client = profile.id.as_str();
    // `first`: the first entry from one click, so the addons go in if they aren't yet
    addons_before_launch_now(core, client, first);
    let p = world_paths(k);
    let st = eng::status(&p);
    if !st.set_up {
        return Err(tag("other", format!("Set {} up first: press Play on its card.", k.title())));
    }
    if !st.running {
        return Err(tag("other", format!("{} isn't running: press Play on its card.", k.title())));
    }
    let (mut pick, mut quick) = (pick, quick);
    if let Some(n) = &pick {
        if !crate::hybrid::char_name_ok(n) {
            return Err(tag("other", "That character's name doesn't look right."));
        }
    }
    let s = eng::load_state(&p);
    let mut is_hero = false;
    let mut elsewhere = false;
    let (mut user, mut pass) = if k == eng::Kind::BossRush {
        // the heroes' account (bossrush_play makes it with the first fight); its password set again, as there
        if s.hero_password.len() < 8 {
            return Err(tag("other", "Pick a fight on the Boss Rush page first: it makes your hero."));
        }
        let hero = br::hero_account(&s.account);
        let [create, set] = br::account_lines(&hero, &s.hero_password).map_err(|e| tag("login", e))?;
        let _ = eng::console(&p, &create);
        eng::console(&p, &set).map_err(|e| tag("login", format!("Boss Rush's realm didn't take the hero's login ({e}), so try again in a minute.")))?;
        (hero, s.hero_password.clone())
    } else {
        // the login healed first: whatever happened to the world's database, the stored login works again
        eng::resync_login(&p).map_err(|e| tag("login", format!("{}'s login couldn't be set up again ({e}), so press Repair on its card, then Play.", k.title())))?;
        match &hero {
            // the Hero Key's own login, with a password for this Play only (its characters moved to it the first time)
            Some(h) => {
                let pass = eng::hero_login(&p, h)?;
                is_hero = true;
                (h.clone(), pass)
            }
            None => (s.account.clone(), s.password.clone()),
        }
    };
    // the characters never "vanish" with the sign-in (signed out, a session the website dropped, another Hero Key on
    // this PC): Play goes in on whichever of the player's own logins here holds them (a picked one's own login), with
    // a password for this Play set over local SQL (no network)
    if k != eng::Kind::BossRush {
        let s_now = eng::load_state(&p);
        if let Ok(logins) = eng::logins_with_chars(&p, &s_now, &user) {
            let to = eng::pick_login(&user, pick.as_deref(), &logins);
            if !to.eq_ignore_ascii_case(&user) {
                pass = if to.eq_ignore_ascii_case(&s_now.account) { s_now.password.clone() } else { eng::login_password(&p, &to)? };
                eng::setup_log(&p, "Play went in on the login that holds the characters (not the one signed in)");
                is_hero = !to.eq_ignore_ascii_case(&s_now.account);
                user = to;
                elsewhere = true;
            }
        }
    }
    if k != eng::Kind::BossRush {
        let mine = || eng::chars_of(&p, &user);
        if first && pick.is_none() {
            match mine() {
                Ok(c) if c.is_empty() => {
                    if quick.is_none() {
                        quick = Some(random_pick(k == eng::Kind::Vanilla)?);
                    }
                }
                // characters here already: the last one played, not a new one
                Ok(_) => quick = None,
                Err(e) => return Err(tag("db", e)),
            }
        }
        if pick.is_none() && quick.is_none() {
            let last = eng::load_state(&p).last_character;
            if !last.is_empty() && mine().map(|c| c.iter().any(|x| x.name.eq_ignore_ascii_case(&last))).unwrap_or(false) {
                pick = Some(last);
            }
        }
    }
    let quick_ids = quick.as_ref().map(|q| (q.race, q.class, q.female.unwrap_or(false)));
    let make = match &quick {
        Some(q) if k != eng::Kind::BossRush => {
            let (race, class) = lw::quick_ok(q.race, q.class, k == eng::Kind::Vanilla).map_err(|e| tag("other", e))?;
            let taken: Vec<String> = eng::refresh_chars(&p).unwrap_or_else(|_| lw::read_kept(&eng::chars_file(&p)).chars).into_iter().map(|c| c.name).collect();
            let name = lw::name_for(q.name.as_deref().unwrap_or(""), &taken).map_err(|e| tag("other", e))?;
            if taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
                return Err(tag("other", format!("You already have a character named {name} here, so pick another name.")));
            }
            Some(crate::hybrid::NewChar { name, race: race.into(), class: class.into(), female: q.female.unwrap_or(false) })
        }
        Some(_) => return Err(tag("other", "Boss Rush's heroes are made on its own page.")),
        None => None,
    };
    let made = make.as_ref().map(|m| m.name.clone()).unwrap_or_default();
    let playing = make.as_ref().map(|m| m.name.clone()).or_else(|| pick.clone());
    let realm = k.realm_name();
    let _ = crate::herokey::set_login(&profile.path, &user, Some(realm));
    let refused = crate::hybrid::refused(&profile.path);
    let enter = pick.is_some() || make.is_some();
    let session = (!refused).then(|| crate::hybrid::Session { user: user.clone(), pass: pass.clone(), realm: Some(realm.into()), enter, pick, make });
    let (message, hybrid) = point_and_play_with(core, client, eng::REALM, true, session, true).map_err(|e| tag("other", e))?;
    let mut auto = hybrid.is_ok();
    let mut typed = false;
    // Leyline's "Logging you in…" cover over the game's own login screen while it is signed in (crate::cover, owner
    // 12 Nov), from the window opening to the world (or character select); Settings' signin_cover switch, Windows only
    let cover_for = |pid: u32| match app {
        Some(a) if k != eng::Kind::BossRush && core.snapshot().signin_cover => cover_start(a, pid, crate::cover::Mode::Single, &user, None),
        _ => None,
    };
    if auto && k != eng::Kind::BossRush {
        // our login screen code signs in: the cover follows the world's database only, typing nothing
        let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
        if let Some(sig) = cover_for(pid) {
            let (wp, wu) = (p.clone(), user.clone());
            std::thread::spawn(move || crate::host::localclick::follow(&wp, &wu, pid, enter, &sig));
        }
    }
    // A game without Leyline's login screen code (a stock game folder): the launcher types the login, Quick start's
    // name on a first entry, and Enter World, as a player would (host/localclick.rs). Boss Rush has its own.
    if !auto && k != eng::Kind::BossRush && crate::host::localclick::can_type() {
        let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
        if pid != 0 {
            // first entry: the world makes the character now (the game's character select can't open its create
            // screen by a key); typed into the create screen only if that fails
            let mut name_to_type = (!made.is_empty()).then(|| made.clone());
            if let (Some(n), Some((race, class, female))) = (name_to_type.clone(), quick_ids) {
                if crate::host::localclick::quick_copy(&p, &user, &n, race, class, female).is_ok() {
                    name_to_type = None;
                }
            }
            let (wp, wu, wpass, wname) = (p.clone(), user.clone(), pass.clone(), name_to_type);
            let cover = cover_for(pid);
            std::thread::spawn(move || {
                let _ = crate::host::localclick::drive(&wp, &wu, &wpass, wname.as_deref(), pid, cover.as_deref());
            });
            auto = true;
            typed = true;
        }
    }
    if let Some(n) = &playing {
        eng::set_last_character(&p, n);
    }
    let plays = WORLD_PLAYS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    stop_world_when_closed(core.dir.clone(), k, LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed), plays);
    // typed by the launcher: the login stays at hand in case the game window didn't take the keys
    Ok(WorldPlay { message, kind: k.word().into(), auto, account: user, password: if auto && !typed { String::new() } else { pass }, made, hero: is_hero, elsewhere })
}

/// Quick start's pick for a first entry the page sent none for: a random race, one of its classes, a free name.
fn random_pick(first_era: bool) -> Result<QuickPick, String> {
    use crate::host::localworld as lw;
    let roll = |n: usize| -> usize {
        use ring::rand::SecureRandom;
        let mut b = [0u8; 4];
        let _ = ring::rand::SystemRandom::new().fill(&mut b);
        (u32::from_le_bytes(b) as usize) % n.max(1)
    };
    let races = lw::choices(first_era);
    let r = races.get(roll(races.len())).ok_or_else(|| crate::host::oneclick::tag("other", "No race to pick from."))?;
    let (class, _) = *r.classes.get(roll(r.classes.len())).ok_or_else(|| crate::host::oneclick::tag("other", "No class to pick from."))?;
    Ok(QuickPick { name: None, race: r.id, class, female: Some(roll(2) == 1) })
}

/// Stop a world a minute after the game the launcher started for it closes (Settings › world_stop_on_exit), unless
/// friends are in it, a Play came since, or another game of ours is running (oneclick::stop_after_exit).
fn stop_world_when_closed(dir: PathBuf, k: crate::host::engine::Kind, pid: u32, plays: u64) {
    use crate::denclick::{Game, Process};
    use crate::host::{engine as eng, oneclick as oc};
    if pid == 0 {
        return;
    }
    std::thread::spawn(move || {
        while Process(pid).alive() {
            std::thread::sleep(Duration::from_secs(2));
        }
        std::thread::sleep(Duration::from_secs(oc::STOP_AFTER_SECS));
        let setting = settings::load(&dir).world_stop_on_exit;
        let last = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
        let other = last != 0 && last != pid && Process(last).alive();
        let p = world_paths(k);
        let up = eng::status(&p).running && !eng::progress().running;
        let friends = if up { eng::friends_online(&p).ok() } else { None };
        if oc::stop_after_exit(setting, plays, WORLD_PLAYS.load(std::sync::atomic::Ordering::SeqCst), other, up, friends) {
            eng::setup_log(&p, "the game closed a minute ago: saving and stopping the world");
            world_share_off(k);
            let _ = eng::stop(&p);
        }
    });
}

#[derive(Serialize)]
struct WorldChars {
    kind: &'static str,
    title: &'static str,
    realm: &'static str,
    supported: bool,
    set_up: bool,
    running: bool,
    /// The world running on this PC now when it is another one (one at a time).
    other_running: Option<crate::host::engine::Kind>,
    /// Read from the world just now (else: kept from when it last ran).
    live: bool,
    at: u64,
    chars: Vec<crate::host::localworld::LocalChar>,
    /// Friends' characters on this world ("Friends": their lasting logins, host/worldfriends.rs), while it runs.
    friends: Vec<crate::host::engine::FriendChar>,
}

/// The Characters page's "On this PC": each local world and its characters (read now from the one running, the
/// others as they were when they last ran).
#[tauri::command]
async fn local_characters() -> Result<Vec<WorldChars>, String> {
    use crate::host::{engine as eng, localworld as lw};
    blocking(|| {
        Ok(eng::KINDS
            .into_iter()
            .map(|k| {
                let p = world_paths(k);
                let st = eng::status(&p);
                let fresh = if st.running { eng::refresh_chars(&p).ok() } else { None };
                let live = fresh.is_some();
                let friends = if st.running { eng::friend_chars(&p).unwrap_or_default() } else { Vec::new() };
                let kept = lw::read_kept(&eng::chars_file(&p));
                WorldChars {
                    kind: k.word(),
                    title: k.title(),
                    realm: k.realm_name(),
                    supported: st.supported,
                    set_up: st.set_up,
                    running: st.running,
                    other_running: st.other_running,
                    live,
                    at: kept.at,
                    chars: fresh.unwrap_or(kept.chars),
                    friends,
                }
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
struct OnlineChars {
    realm: &'static str,
    chars: Vec<crate::host::localworld::OnlineChar>,
}

/// The Characters page's "Online": the characters the player's game folders remember on the official realm (their
/// own settings folders; the realm itself keeps the characters).
#[tauri::command]
fn online_characters(core: State<Core>) -> Result<OnlineChars, String> {
    let s = core.snapshot();
    let mut chars: Vec<crate::host::localworld::OnlineChar> = Vec::new();
    for c in s.clients.iter().filter(|c| c.build == clients::Build::V335a) {
        for x in crate::host::localworld::online_chars(&c.path, &[realms::SAMPLE_NAME]) {
            if !chars.contains(&x) {
                chars.push(x);
            }
        }
    }
    Ok(OnlineChars { realm: realms::SAMPLE_NAME, chars })
}

/// Invite a friend into a single player world while it runs (host/worldshare.rs): its own key on the Leyline relay
/// (private, the tunnel Your Den uses: one at a time on this PC), the same network through the launcher's own
/// forwarder, and one friend's login (a player, made over the world's console) in the link.
#[tauri::command]
async fn world_invite(kind: String) -> Result<DenInvite, String> {
    let k = world_kind(&kind)?;
    blocking(move || world_invite_now(k)).await
}

fn world_invite_now(k: crate::host::engine::Kind) -> Result<DenInvite, String> {
    use crate::host::{denhost, denways, engine as eng, litenet, worldfriends as wf, worldshare as ws};
    if k == eng::Kind::BossRush && !bossrush_friend_file().is_file() {
        return Err("Pick the fight for you and your friend on the Boss Rush page first (With friends).".into());
    }
    let p = world_paths(k);
    if !eng::status(&p).running {
        return Err(format!("Start {} first: friends join it while it runs.", k.title()));
    }
    let share = ws::dir(&p.root);
    std::fs::create_dir_all(&share).map_err(|e| e.to_string())?;
    let seed = litenet::load_or_create_key(&litenet::key_path(&share))?;
    let secret = ws::invite_secret(&share)?;
    let mine = litenet::realm_id(&litenet::public_key(&seed)?);
    // friends' launchers claim their lasting login in front of the login server (host/worldfriends.rs): the tunnel
    // and the same-network way go through it; the game's own bytes pass through untouched
    let fw = world_friends_of(k, &mine, &secret);
    let _ = wf::sweep(&fw);
    let front = wf::front_start(eng::LOGIN_PORT, fw)?;
    std::fs::write(share.join("den-lite.remote"), format!("{front} {}\n", eng::WORLD_PORT)).map_err(|e| e.to_string())?;
    ingame_sync_start();
    // one tunnel on this PC: another world's (or Your Den's, which can't run beside a world: the same login port)
    // gives way
    let now = litenet::state();
    if now.running && now.realm_id != mine {
        litenet::stop();
        std::thread::sleep(Duration::from_millis(300));
    }
    let mut tunnel = litenet::start(litenet::Setup {
        endpoint: litenet::Endpoint::relay(realms::RELAY),
        relay_name: realms::RELAY.into(),
        seed,
        name: k.realm_name().into(),
        invite_secret: secret.clone(),
        den_dir: share.clone(),
        realm_list: true,
    });
    for _ in 0..40 {
        if tunnel.connected || !tunnel.last_error.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
        tunnel = litenet::state();
    }
    // the friend's login
    let (user, pass) = ws::new_guest()?;
    let [create, addon] = ws::guest_lines(&user, &pass)?;
    ws::created(&eng::console(&p, &create)?).map_err(|e| format!("The world didn't make your friend's login: {e}"))?;
    eng::console(&p, &addon)?;
    // the link's one-time login is the ticket a new friend claims their lasting login with
    wf::add_ticket(&share, &user, &pass, now_secs())?;
    // the same network
    let (lan, lan_way) = match denways::lan_address() {
        None => (None, InviteWay { id: "lan", ok: false, line: "Same network: this PC isn't on a home or office network.".into() }),
        Some(ip) => match ws::lan_start_via(ip, eng::LOGIN_PORT, front, eng::WORLD_PORT, ws::Layout::Wrath) {
            Ok((ip, lp, _)) => (Some(format!("{ip}:{lp}")), InviteWay { id: "lan", ok: true, line: format!("Same network: on, at {ip} (friends at your home or on your network come in here first).") }),
            Err(e) => (None, InviteWay { id: "lan", ok: false, line: format!("Same network: off ({e}). Allow Leyline on private networks in your firewall, then Invite again.") }),
        },
    };
    let relay_way = if tunnel.last_error.is_empty() {
        InviteWay { id: "relay", ok: true, line: "Leyline relay: on (works from anywhere; the relay only passes the game's connection through).".into() }
    } else {
        InviteWay { id: "relay", ok: false, line: format!("Leyline relay: {}", tunnel.last_error) }
    };
    let own_way = InviteWay { id: "own", ok: false, line: "Your own address: not used for a single player world.".into() };
    let link = litenet::invite_link(realms::RELAY, &mine, &secret, Some((&user, &pass)));
    let code = denhost::code_from_link(&link).unwrap_or_default();
    let link = wf::with_mark(&denways::with_ways(&link, lan.as_deref(), None));
    // the game's Leyline tab shows it to the host (ov_world_share)
    let _ = eng::sql(&p, "acore_characters", &format!("{} {}", wf::INGAME_TABLES, wf::share_sql(true, &link, now_secs())));
    Ok(DenInvite { link, code, name: "your friend".into(), expires: now_secs() + wf::TICKET_DAYS * 86_400, tunnel, ways: vec![lan_way, own_way, relay_way] })
}

/// Sharing a world ends: the same-network way, and its tunnel when the tunnel is this world's; friends' lasting
/// logins disabled until they are invited again (or trusted), one-time logins with no characters removed
/// (host/worldfriends.rs).
fn world_share_off(k: crate::host::engine::Kind) {
    use crate::host::{engine as eng, litenet, worldfriends as wf, worldshare as ws};
    let share = ws::dir(&world_paths(k).root);
    let mine = litenet::load_or_create_key(&litenet::key_path(&share)).and_then(|s| litenet::public_key(&s)).map(|p| litenet::realm_id(&p)).ok();
    if mine.as_ref().is_some_and(|m| &litenet::state().realm_id == m) {
        litenet::stop();
    }
    ws::lan_stop();
    wf::front_stop();
    if let Some(w) = world_friends_here(k) {
        wf::share_stopped(&w);
    }
    friends_world_closed(k.word());
    let _ = eng::sql(&world_paths(k), "acore_characters", &format!("{} {}", wf::INGAME_TABLES, wf::share_sql(false, "", now_secs())));
}

/// A world's sharing stopped: friends on the Leyline network stop seeing it open (their next presence, sent now).
fn friends_world_closed(kind: &str) {
    if let Some(h) = crate::p2p::friendnet::current() {
        if h.world_closed(kind) && h.state.lock().map(|s| s.running).unwrap_or(false) {
            std::thread::spawn(move || h.presence_all(true));
        }
    }
}

/// A Built-in engine world as its friends' claims need it (host/worldfriends.rs).
fn world_friends_of(k: crate::host::engine::Kind, realm_id: &str, secret: &str) -> crate::host::worldfriends::World {
    use crate::host::{engine as eng, worldfriends as wf, worldshare as ws};
    wf::World {
        realm_id: realm_id.into(),
        secret: secret.into(),
        share: ws::dir(&world_paths(k).root),
        era: wf::Era::Wrath,
        console: Arc::new(move |l: &str| eng::console(&world_paths(k), l)),
        // an error is passed on: no friend's login is deleted when the world couldn't say who has characters
        with_chars: Arc::new(move || eng::friend_chars(&world_paths(k)).map(|v| v.into_iter().map(|c| c.user).collect())),
        // Boss Rush: a friend plays a hero (the module's heroes live on their own account), made as they come in
        login_for: (k == eng::Kind::BossRush).then(|| -> wf::LoginFor { Arc::new(|u: &str, p: &str| bossrush_friend_login(u, p)) }),
    }
}

/// [`world_friends_of`] for a world that has been shared before (its key and invite secret are there; never made here).
fn world_friends_here(k: crate::host::engine::Kind) -> Option<crate::host::worldfriends::World> {
    use crate::host::{litenet, worldshare as ws};
    let share = ws::dir(&world_paths(k).root);
    if !litenet::key_path(&share).is_file() || !share.join("invite.secret").is_file() {
        return None;
    }
    let rid = litenet::realm_id(&litenet::public_key(&litenet::load_or_create_key(&litenet::key_path(&share)).ok()?).ok()?);
    let secret = ws::invite_secret(&share).ok()?;
    Some(world_friends_of(k, &rid, &secret))
}

/// Leyline Solo as its friends' claims need it.
fn classic_friends_of(realm_id: &str, secret: &str) -> crate::host::worldfriends::World {
    use crate::host::{classic, worldfriends as wf};
    let p = classic_paths();
    wf::World {
        realm_id: realm_id.into(),
        secret: secret.into(),
        share: p.share.clone(),
        era: wf::Era::Classic,
        console: Arc::new(|l: &str| classic::soap(&classic::load_state(&classic_paths()), l)),
        with_chars: Arc::new(|| classic::friend_logins_with_chars(&classic_paths())),
        login_for: None,
    }
}

fn classic_friends_here() -> Option<crate::host::worldfriends::World> {
    use crate::host::{litenet, worldshare as ws};
    let share = classic_paths().share;
    if !litenet::key_path(&share).is_file() || !share.join("invite.secret").is_file() {
        return None;
    }
    let rid = litenet::realm_id(&litenet::public_key(&litenet::load_or_create_key(&litenet::key_path(&share)).ok()?).ok()?);
    let secret = ws::invite_secret(&share).ok()?;
    Some(classic_friends_of(&rid, &secret))
}

/// The game's Leyline tab (WrathUnleashedUI/WorldFriends.lua <-> lua_scripts/ov_worldfriends.lua): while a Built-in
/// engine world runs, the launcher answers its asks (the host's "Invite a friend") and keeps its friends table and
/// sharing state current, through the world's own database (host/worldfriends.rs, INGAME_TABLES). One thread; it
/// ends a minute after no world runs, and the next world status or start begins it again.
fn ingame_sync_start() {
    use crate::host::{engine as eng, worldfriends as wf, worldshare as ws};
    use std::sync::atomic::{AtomicBool, Ordering};
    static ON: AtomicBool = AtomicBool::new(false);
    if ON.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        let (mut idle, mut made, mut last) = (0u32, None::<eng::Kind>, String::new());
        loop {
            std::thread::sleep(Duration::from_secs(4));
            let k = eng::active().filter(|k| *k != eng::Kind::BossRush);
            let Some(k) = k.filter(|k| eng::status(&world_paths(*k)).running) else {
                idle += 1;
                made = None;
                if idle > 15 {
                    break;
                }
                continue;
            };
            idle = 0;
            let p = world_paths(k);
            if made != Some(k) {
                if eng::sql(&p, "acore_characters", wf::INGAME_TABLES).is_err() {
                    continue;
                }
                let _ = eng::sql(&p, "acore_characters", &wf::expire_sql(now_secs()));
                made = Some(k);
                last.clear();
            }
            if let Ok(rows) = eng::query(&p, "acore_characters", wf::ASKS_SQL) {
                for (id, kind) in wf::parse_asks(&rows) {
                    let (ok, text) = match kind.as_str() {
                        "invite" => match world_invite_now(k) {
                            Ok(_) => (true, "Your invite link is ready: copy it and send it to your friend.".to_string()),
                            Err(e) => (false, e),
                        },
                        _ => (false, "The launcher doesn't know that ask.".to_string()),
                    };
                    let _ = eng::sql(&p, "acore_characters", &wf::done_sql(id, ok, &text));
                }
            }
            let friends = wf::load(&ws::dir(&p.root)).friends;
            let now = format!("{} {}", wf::friends_sql(&friends), wf::sharing_sql(wf::front_running(), 0));
            if now != last && eng::sql(&p, "acore_characters", &now).is_ok() {
                last = now;
            }
        }
        ON.store(false, Ordering::SeqCst);
    });
}

#[derive(Serialize)]
struct WorldFriendView {
    user: String,
    /// The friend's peer id on the Leyline network ("" for an earlier friend's login not yet taken over).
    peer: String,
    /// Their name when they are on this player's Friends list (p2p/friends.rs), else "".
    name: String,
    /// On this player's Friends list, and online there now.
    on_list: bool,
    online: bool,
    trusted: bool,
    /// Invited now (can come in while the world is shared).
    enabled: bool,
    /// An earlier one-time login kept for its characters (not yet taken over by its friend's launcher).
    legacy: bool,
    seen: u64,
    chars: Vec<crate::host::engine::FriendChar>,
}

/// The friends with a lasting login on a world ("single", "vanilla" or "solo"), with their characters (read now
/// while it runs, else as kept when it last ran).
#[tauri::command]
async fn world_friends(kind: String) -> Result<Vec<WorldFriendView>, String> {
    use crate::host::{classic, engine as eng, worldfriends as wf, worldshare as ws};
    blocking(move || {
        let (share, chars) = if kind == "solo" {
            let p = classic_paths();
            (p.share.clone(), classic::friend_chars(&p))
        } else {
            let k = world_kind(&kind)?;
            let p = world_paths(k);
            (ws::dir(&p.root), if eng::status(&p).running { eng::friend_chars(&p).unwrap_or_default() } else { Vec::new() })
        };
        let mut b = wf::load(&share);
        let users: Vec<String> = chars.iter().map(|c| c.user.clone()).collect();
        wf::fold(&mut b, &users, now_secs());
        // the same identity as the Friends list (identity.rs): its names, and who is online there
        let known: Vec<(String, String, bool)> = crate::p2p::friendnet::current()
            .map(|h| {
                let v = h.book.lock().unwrap().view(&h.me, crate::p2p::friendnet::now(), &[], crate::p2p::friendnet::ONLINE_WINDOW);
                v["friends"].as_array().into_iter().flatten()
                    .map(|f| (f["peer_id"].as_str().unwrap_or("").to_string(), f["name"].as_str().unwrap_or("").to_string(), f["online"].as_bool().unwrap_or(false)))
                    .collect()
            })
            .unwrap_or_default();
        Ok(b.friends
            .into_iter()
            .map(|f| {
                let k = known.iter().find(|(p, _, _)| !f.peer.is_empty() && *p == f.peer);
                WorldFriendView {
                chars: chars.iter().filter(|c| c.user == f.user).cloned().collect(),
                peer: f.peer.clone(),
                name: k.map(|x| x.1.clone()).unwrap_or_default(),
                on_list: k.is_some(),
                online: k.is_some_and(|x| x.2),
                user: f.user,
                trusted: f.trusted,
                enabled: f.enabled,
                legacy: f.legacy,
                seen: f.seen,
            }})
            .collect())
    })
    .await
}

/// The host's choice on one friend: "trust", "untrust", "remove" (their characters kept) or "delete" (with them).
#[tauri::command]
async fn world_friend(kind: String, user: String, action: String) -> Result<String, String> {
    blocking(move || {
        // "give:<login>": the host gives this friend's characters to another friend's login (a friend who came back
        // with a new Leyline key: a reinstall, a new PC), on the Built-in engine's worlds
        if let Some(to) = action.strip_prefix("give:") {
            use crate::host::engine as eng;
            if kind == "solo" {
                return Err("Giving characters to another friend works on Single player and the original world.".to_string());
            }
            let k = world_kind(&kind)?;
            let p = world_paths(k);
            if !eng::status(&p).running {
                return Err("Your world has to be running for that (start it, then try again).".to_string());
            }
            let book = crate::host::worldfriends::load(&crate::host::worldshare::dir(&p.root));
            if !book.friends.iter().any(|f| f.user == to) {
                return Err("That friend isn't on this world.".to_string());
            }
            let n = eng::give_chars(&p, &user, to)?;
            return Ok(if n == 0 { "They have no characters here to give.".to_string() } else { format!("{n} characters given. A back-up was taken first.") });
        }
        let w = if kind == "solo" { classic_friends_here() } else { world_friends_here(world_kind(&kind)?) };
        let w = w.ok_or("Invite a friend first: this world has no friends yet.")?;
        crate::host::worldfriends::set_friend(&w, &user, &action)
    })
    .await
}

/// The friend's side: the worlds this launcher joined (for "Join your friend again").
#[tauri::command]
fn friend_worlds() -> Vec<crate::host::worldfriends::JoinedView> {
    crate::host::worldfriends::worlds_view(&settings::app_dir())
}

/// The friend's side: forget a remembered world on this PC (the friend's world keeps the character).
#[tauri::command]
fn friend_world_forget(realm_id: String) -> Result<(), String> {
    crate::host::worldfriends::forget(&settings::app_dir(), &realm_id)
}

/// Join a remembered friend's world again, with no new link: this launcher's identity claims its lasting login
/// there (while the friend shares the world and has invited or trusted it).
#[tauri::command]
async fn join_friend_again(core: State<'_, Core>, client: String, realm_id: String) -> Result<Joined, String> {
    use crate::host::worldfriends as wf;
    let j = wf::worlds(&settings::app_dir()).remove(&realm_id).ok_or("This PC hasn't joined that world.")?;
    join_invite(core, client, wf::again_link(&realm_id, &j)).await
}

#[tauri::command]
async fn world_share_stop(kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || {
        world_share_off(k);
        Ok("Sharing stopped: friends' links don't work until you invite again.".to_string())
    })
    .await
}

// ---- Friends on the Leyline network, no server (p2p/friends.rs, p2p/friendnet.rs, identity.rs; docs/LEYLINE-FRIENDS.md;
// ui/friends.js). Everything is signed by each player's own peer key and kept in <app>/cache/friends.json.

fn friends_hub() -> Result<std::sync::Arc<crate::p2p::friendnet::Hub>, String> {
    crate::p2p::friendnet::hub(&settings::app_dir())
}

/// The Friends page: me, friends (online first), requests both ways, people near, the network's state.
#[tauri::command]
async fn friends_state() -> Result<serde_json::Value, String> {
    blocking(|| {
        use crate::p2p::friendnet::{now, ONLINE_WINDOW};
        let h = friends_hub()?;
        let t = now();
        let near = h.near_list(t);
        let mut v = h.book.lock().unwrap().view(&h.me, t, &near, ONLINE_WINDOW);
        v["net"] = serde_json::to_value(h.state.lock().unwrap().clone()).unwrap_or_default();
        Ok(v)
    })
    .await
}

/// Friends on: listening on this network, beacons, presence, the mailbox (once; the page calls it when it loads).
#[tauri::command]
async fn friends_start() -> Result<crate::p2p::friendnet::NetState, String> {
    blocking(|| Ok(crate::p2p::friendnet::start(&friends_hub()?))).await
}

#[tauri::command]
async fn friends_stop() -> Result<(), String> {
    blocking(|| {
        crate::p2p::friendnet::stop(&friends_hub()?);
        Ok(())
    })
    .await
}

/// Your profile: name, status, sharing what you play, how you're seen on this network, the relay mailbox.
#[tauri::command]
async fn friends_profile(name: String, status: String, share_playing: bool, lan: String, mailbox: bool) -> Result<(), String> {
    blocking(move || {
        let h = friends_hub()?;
        {
            let mut b = h.book.lock().unwrap();
            let p = &mut b.profile;
            p.name = name.chars().take(32).collect();
            p.status = status.chars().take(80).collect();
            p.share_playing = share_playing;
            p.lan = if ["off", "friends", "everyone"].contains(&lan.as_str()) { lan } else { "friends".into() };
            p.mailbox = mailbox;
            p.seq += 1;
        }
        h.save()?;
        // friends get the new card with the next presence; now, so the name changes at once
        std::thread::spawn(move || h.presence_all(true));
        Ok(())
    })
    .await
}

/// A new friend code to give someone (good for a day).
#[tauri::command]
async fn friends_code() -> Result<String, String> {
    blocking(|| {
        let h = friends_hub()?;
        let c = h.book.lock().unwrap().new_code(&h.me, crate::p2p::friendnet::now())?;
        h.save()?;
        Ok(c)
    })
    .await
}

/// Add a friend by their code: the request goes out (same network, or their relay's mailbox). Returns the way.
#[tauri::command]
async fn friends_add(code: String) -> Result<String, String> {
    blocking(move || {
        let h = friends_hub()?;
        let c = crate::p2p::friends::parse_code(&code)?;
        let m = h.book.lock().unwrap().request(&h.me, &c.pubkey, Some(&c.token), c.relay.as_deref(), crate::p2p::friendnet::now())?;
        h.save()?;
        match h.deliver(&identity_hex(&c.pubkey), &m, c.relay.as_deref()) {
            Ok(way) => Ok(way.to_string()),
            Err(_) => Ok("waiting".to_string()), // kept, and sent again when Leyline next starts or they come near
        }
    })
    .await
}

fn identity_hex(k: &[u8; 32]) -> String {
    crate::identity::hex(k)
}

/// Ask someone near (on this network, showing their name) to be friends.
#[tauri::command]
async fn friends_ask_near(peer_id: String) -> Result<String, String> {
    blocking(move || {
        let h = friends_hub()?;
        let key = h.near_list(crate::p2p::friendnet::now()).into_iter().map(|(k, _)| k).find(|k| crate::identity::unhex(k).map(|b| crate::identity::peer_id(&b)).as_deref() == Some(peer_id.as_str())).ok_or("They aren't on your network any more.")?;
        let pk = crate::identity::unhex32(&key).ok_or("bad key")?;
        let m = h.book.lock().unwrap().request(&h.me, &pk, None, None, crate::p2p::friendnet::now())?;
        h.save()?;
        h.deliver(&key, &m, None).map(String::from)
    })
    .await
}

/// accept, decline, cancel (my request) or remove (a friend) by peer id.
#[tauri::command]
async fn friends_answer(peer_id: String, action: String) -> Result<String, String> {
    blocking(move || {
        let h = friends_hub()?;
        let t = crate::p2p::friendnet::now();
        let out = {
            let mut b = h.book.lock().unwrap();
            match action.as_str() {
                "accept" => Some(b.accept(&h.me, &peer_id, t)?),
                "decline" => Some(b.decline(&h.me, &peer_id, t)?),
                "remove" => Some(b.revoke(&h.me, &peer_id, t)?),
                "cancel" => {
                    b.cancel(&peer_id);
                    None
                }
                _ => return Err("[other] Unknown action.".into()),
            }
        };
        h.save()?;
        let Some((to, m)) = out else { return Ok("done".into()) };
        let relay = h.book.lock().unwrap().mailbox_of(&h.me, &peer_id);
        Ok(h.deliver(&to, &m, relay.as_deref()).map(String::from).unwrap_or_else(|_| "here".into()))
    })
    .await
}

/// Invite a friend into a running world of this PC (single or vanilla): the world's invite (world_invite) sent to
/// them alone, signed and sealed.
#[tauri::command]
async fn friends_invite(peer_id: String, kind: String) -> Result<String, String> {
    let k = world_kind(&kind)?;
    blocking(move || {
        let h = friends_hub()?;
        let key = h.book.lock().unwrap().friend_key(&h.me, &peer_id).ok_or("That friend isn't on your list.")?;
        let inv = world_invite_now(k)?;
        let body = serde_json::json!({"world": {"kind": k.word(), "name": k.title()}, "link": inv.link});
        let m = crate::p2p::friends::make_msg(&h.me, &key, "invite", body, crate::p2p::friendnet::now())?;
        *h.playing.lock().unwrap() = (Some(k.title().to_string()), Some(serde_json::json!({"kind": k.word(), "name": k.title(), "invite_hint": "friends"})));
        h.deliver(&key, &m, None).map(String::from)
    })
    .await
}

/// A friend's latest invite link (the page then shows the usual Join: solo.js offerJoin).
#[tauri::command]
async fn friends_join(peer_id: String) -> Result<String, String> {
    blocking(move || {
        let h = friends_hub()?;
        let b = h.book.lock().unwrap();
        let mine = h.me.public_hex();
        let f = b.friends.iter().find(|f| {
            let d = &f.friendship["doc"];
            let other = if d["a"].as_str() == Some(mine.as_str()) { &d["b"] } else { &d["a"] };
            other.as_str().and_then(crate::identity::unhex).map(|k| crate::identity::peer_id(&k)).as_deref() == Some(peer_id.as_str())
        });
        let link = f.and_then(|f| f.invite.as_ref()).and_then(|m| m["msg"]["body"]["link"].as_str()).map(String::from);
        link.ok_or_else(|| "No invite from them yet: ask them to press Invite on your name.".to_string())
    })
    .await
}

// ---- Leyline Solo (host/classic.rs, host/classicsign.rs; docs/LEYLINE-SOLO.md: the contract the Solo app codes
// against): the first-era world on this PC, its fast sign-in, its characters and companions, Invite a friend.

fn classic_paths() -> crate::host::classic::Paths {
    crate::host::classic::Paths::new(&settings::app_dir())
}

/// A game folder of the first era, by its id.
fn classic_client(core: &Core, client: &str) -> Result<ClientProfile, String> {
    let profile = core.client(client)?;
    if profile.build != clients::Build::V1121 {
        return Err("Leyline Solo plays with your first-era game folder: pick that one.".into());
    }
    Ok(profile)
}

#[tauri::command]
async fn classic_status() -> Result<crate::host::classic::ClassicStatus, String> {
    blocking(|| {
        let p = classic_paths();
        let st = crate::host::classic::status(&p);
        // friends' logins whose time ran out go while the world runs (host/classic.rs sweep_friends)
        if st.running && st.friends > 0 {
            let mut s = crate::host::classic::load_state(&p);
            if s.friends.iter().any(|f| f.expires <= now_secs()) {
                crate::host::classic::sweep_friends(&p, &mut s, now_secs(), false);
                return Ok(crate::host::classic::status(&p));
            }
        }
        Ok(st)
    })
    .await
}

/// Set Leyline Solo up (whatever isn't done: the engine, the maps from this first-era folder, the database, the
/// accounts) and start it; follow it with classic_progress. `mmaps`: the monster paths too (1 to 3 hours).
#[tauri::command]
fn classic_setup(core: State<Core>, client: String, mmaps: Option<bool>, update: Option<bool>) -> Result<String, String> {
    let profile = classic_client(&core, &client)?;
    crate::host::classic::begin(&settings::app_dir(), Some(profile.path), mmaps.unwrap_or(false), update.unwrap_or(false))
}

/// Start it (set up already). `roaming`: other adventurers roam the world (taken now and kept). `stopOther`: another
/// world on these ports is stopped first (else the answer says "ports busy").
#[tauri::command]
async fn classic_start(roaming: Option<bool>, stop_other: Option<bool>) -> Result<String, String> {
    blocking(move || {
        if let Some(r) = roaming {
            crate::host::classic::set_roaming(&classic_paths(), r)?;
        }
        if stop_other.unwrap_or(false) {
            stop_other_worlds();
        }
        crate::host::classic::begin(&settings::app_dir(), None, false, false)
    })
    .await
}

/// Whatever else is on Leyline Solo's ports, stopped: the Built-in engine's worlds and Your Den on Den Lite.
fn stop_other_worlds() {
    use crate::host::lite::DenLite;
    let p = classic_paths();
    if crate::host::classic::other_running(&p).is_none() {
        return;
    }
    for k in crate::host::engine::KINDS {
        let wp = world_paths(k);
        if crate::host::engine::active() == Some(k) || crate::host::engine::status(&wp).running {
            world_share_off(k);
            let _ = crate::host::engine::stop(&wp);
        }
    }
    let app = settings::app_dir();
    if crate::host::lite::beat_fresh(&crate::host::dens::dir(&crate::host::lite::root(&app))) {
        let _ = crate::host::lite::Binary::find(&app).stop();
    }
}

#[tauri::command]
fn classic_progress() -> crate::host::engine::Job {
    crate::host::classic::progress()
}

/// Friends' logins removed, everyone saved, everything stopped; sharing ends with it.
#[tauri::command]
async fn classic_stop() -> Result<String, String> {
    blocking(|| {
        classic_share_off();
        crate::host::classic::stop(&classic_paths())
    })
    .await
}

#[tauri::command]
async fn classic_logs(lines: Option<usize>) -> Result<String, String> {
    blocking(move || Ok(crate::host::classic::logs(&classic_paths(), lines.unwrap_or(200).clamp(20, 2000)))).await
}

#[tauri::command]
async fn classic_update_check() -> Result<crate::host::classic::ClassicStatus, String> {
    blocking(|| {
        let p = classic_paths();
        crate::host::classic::check_update(&p)?;
        Ok(crate::host::classic::status(&p))
    })
    .await
}

/// Settings: `signin` ("simple": the account name filled in, the password typed once; "quick": the sign-in add-on,
/// opt-in, TODO(owner) approval) saved as settings.json `solo_signin`; `roaming` (other adventurers in the world)
/// kept in the world's state for its next start. Back to simple takes our archive out of the game folder.
#[tauri::command]
async fn classic_settings(core: State<'_, Core>, signin: Option<String>, roaming: Option<bool>) -> Result<String, String> {
    use crate::host::classicsign::Mode;
    let m = signin.as_deref().map(Mode::parse).transpose()?;
    if let Some(m) = m {
        core.update(|s| {
            s.solo_signin = m.word().into();
            Ok(())
        })?;
    }
    blocking(move || {
        let p = classic_paths();
        if let Some(r) = roaming {
            crate::host::classic::set_roaming(&p, r)?;
        }
        if m == Some(Mode::Prefill) {
            let mut s = crate::host::classic::load_state(&p);
            if s.signin.on && !s.signin.added.is_empty() {
                let game = PathBuf::from(&s.signin.game);
                crate::host::classicsign::install(&game, &p.backup, &mut s.signin, Mode::Prefill, None)?;
                crate::host::classic::save_state(&p, &s)?;
            }
        }
        Ok("Saved.".to_string())
    })
    .await
}

#[derive(Serialize)]
struct ClassicPlay {
    message: String,
    mode: String,
    /// The add-on signs in by itself this time.
    auto: bool,
    account: String,
    /// Prefill (or the add-on couldn't be used): the page offers "Show password" (classic_password).
    password_needed: bool,
    /// Quick start's character's name (the add-on makes it; empty otherwise).
    made: String,
}

/// Play Leyline Solo: the sign-in written (backups first), then the game started from its own folder. `signin`:
/// "simple" or "quick" (else the saved setting). `pick` enters that character, `quick` makes a new one first (both:
/// the quick sign-in only). `stopOther`: another world on these ports is stopped first; without it that is an error
/// saying "ports busy".
#[tauri::command]
async fn classic_play(core: State<'_, Core>, client: String, signin: Option<String>, pick: Option<String>, quick: Option<QuickPick>, stop_other: Option<bool>) -> Result<ClassicPlay, String> {
    use crate::host::{classic, classicsign::Mode, localworld as lw};
    let profile = classic_client(&core, &client)?;
    let p = classic_paths();
    if let Some(o) = classic::other_running(&p) {
        if !stop_other.unwrap_or(false) {
            return Err(format!("{o} is running on this PC and uses the same ports (ports busy)."));
        }
        blocking(|| {
            stop_other_worlds();
            Ok(())
        })
        .await?;
    }
    let st = classic::status(&p);
    if !st.set_up {
        return Err("Set Leyline Solo up first: press Start on its page.".into());
    }
    if !st.running {
        return Err("Leyline Solo isn't running: press Start first.".into());
    }
    if let Some(n) = &pick {
        if !crate::hybrid::char_name_ok(n) {
            return Err("That character's name doesn't look right.".into());
        }
    }
    let mode = match &signin {
        Some(w) => Mode::parse(w)?,
        None => Mode::from_word(&core.snapshot().solo_signin),
    };
    let make = match &quick {
        Some(q) => {
            let (race, class) = lw::quick_ok(q.race, q.class, true)?;
            let taken: Vec<String> = classic::refresh_chars(&p).unwrap_or_else(|_| classic::read_kept(&p).chars).into_iter().map(|c| c.base.name).collect();
            let name = lw::name_for(q.name.as_deref().unwrap_or(""), &taken)?;
            if taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
                return Err(format!("You already have a character named {name} here: pick another name."));
            }
            Some(crate::hybrid::NewChar { name, race: race.into(), class: class.into(), female: q.female.unwrap_or(false) })
        }
        None => None,
    };
    let made = if mode == Mode::Addon { make.as_ref().map(|m| m.name.clone()).unwrap_or_default() } else { String::new() };
    let game = profile.path.clone();
    let signed = blocking(move || classic::sign_in(&classic_paths(), &game, mode, pick, make)).await?;
    let wine = core.snapshot().wine;
    let plan = launch::plan(&profile, &wine, launch::Os::current())?;
    let child = launch::run(&plan).inspect_err(|_| {
        let _ = classic::scrub_signin(&classic_paths());
    })?;
    LAST_GAME_PID.store(child.id(), std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut g) = core.games.lock() {
        g.push(child);
    }
    if signed.auto {
        classic_scrub_when_closed();
    }
    let message = if signed.auto {
        format!("Started {}: it signs in to Leyline Solo by itself.", profile.name)
    } else if signed.why.is_empty() {
        format!("Started {}: the account {} is filled in; type its password once (Show password).", profile.name, classic::ACCOUNT)
    } else {
        format!("Started {}: the account {} is filled in; type its password once (Show password). The sign-in add-on wasn't used: {}.", profile.name, classic::ACCOUNT, signed.why)
    };
    Ok(ClassicPlay { message, mode: signed.mode.ui_word().into(), auto: signed.auto, account: classic::ACCOUNT.into(), password_needed: !signed.auto, made: if signed.auto { made } else { String::new() } })
}

/// The password out of the add-on's archive once the game lets go of it (the add-on read it at the start): first
/// after 90 s, then every 15 s, for at most 12 hours; and on stop.
fn classic_scrub_when_closed() {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(90));
        for _ in 0..(12 * 240) {
            if crate::host::classic::scrub_signin(&classic_paths()).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_secs(15));
        }
    });
}

#[derive(Serialize)]
struct ClassicPassword {
    account: String,
    password: String,
}

/// SOLO's account and password for the page to show once and copy: returned only here, never logged.
#[tauri::command]
fn classic_password() -> Result<ClassicPassword, String> {
    let password = crate::host::classic::password(&classic_paths())?;
    Ok(ClassicPassword { account: crate::host::classic::ACCOUNT.into(), password })
}

/// Sign-in off: every file of the game folder put back as it was, our archive removed (close the game first).
/// `client`, when given, must be the folder the sign-in is in.
#[tauri::command]
async fn classic_signin_off(core: State<'_, Core>, client: Option<String>) -> Result<String, String> {
    if let Some(c) = client.as_deref().filter(|c| !c.is_empty()) {
        let path = classic_client(&core, c)?.path;
        let st = crate::host::classic::load_state(&classic_paths());
        if st.signin.on && PathBuf::from(&st.signin.game) != path {
            return Err("The Solo sign-in is in another game folder: pick that folder, then try again.".into());
        }
    }
    blocking(|| crate::host::classic::signin_off(&classic_paths())).await
}

/// The characters: `[{name, level, race_label, class_label, zone, companion, online, ...}]`.
#[tauri::command]
async fn classic_characters() -> Result<Vec<crate::host::classic::ClassicChar>, String> {
    blocking(|| Ok(crate::host::classic::characters(&classic_paths()).chars)).await
}

/// The logs folder opened in the PC's file manager.
#[tauri::command]
fn classic_open_logs(app: AppHandle) -> Result<String, String> {
    let p = classic_paths();
    std::fs::create_dir_all(&p.logs).map_err(|e| e.to_string())?;
    app.opener().open_path(p.logs.display().to_string(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(p.logs.display().to_string())
}

#[tauri::command]
fn classic_choices() -> crate::host::classic::ClassicChoices {
    crate::host::classic::choices(&classic_paths())
}

/// Invite a friend into Leyline Solo while it runs: one friend's login for 7 days (made over the world's console),
/// the same network through the launcher's forwarder (the first era's realm list), the relay tunnel.
#[tauri::command]
async fn classic_invite() -> Result<DenInvite, String> {
    blocking(classic_invite_now).await
}

fn classic_invite_now() -> Result<DenInvite, String> {
    use crate::host::{classic, denhost, denways, litenet, worldfriends as wf, worldshare as ws};
    let p = classic_paths();
    if !classic::status(&p).running {
        return Err("Start Leyline Solo first: friends join it while it runs.".into());
    }
    classic::share_files(&p)?;
    let seed = litenet::load_or_create_key(&litenet::key_path(&p.share))?;
    let secret = ws::invite_secret(&p.share)?;
    let mine = litenet::realm_id(&litenet::public_key(&seed)?);
    // friends' lasting logins are claimed in front of the login server (host/worldfriends.rs)
    let front = wf::front_start(classic::LOGIN_PORT, classic_friends_of(&mine, &secret))?;
    std::fs::write(p.share.join("den-lite.remote"), format!("{front} {}\n", classic::WORLD_PORT)).map_err(|e| e.to_string())?;
    let now = litenet::state();
    if now.running && now.realm_id != mine {
        litenet::stop();
        std::thread::sleep(Duration::from_millis(300));
    }
    let mut tunnel = litenet::start(litenet::Setup {
        endpoint: litenet::Endpoint::relay(realms::RELAY),
        relay_name: realms::RELAY.into(),
        seed,
        name: classic::REALM_NAME.into(),
        invite_secret: secret.clone(),
        den_dir: p.share.clone(),
        realm_list: true,
    });
    for _ in 0..40 {
        if tunnel.connected || !tunnel.last_error.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
        tunnel = litenet::state();
    }
    let (user, pass, expires) = classic::add_friend(&p)?;
    wf::add_ticket(&p.share, &user, &pass, now_secs())?;
    let (lan, lan_way) = match denways::lan_address() {
        None => (None, InviteWay { id: "lan", ok: false, line: "Same network: this PC isn't on a home or office network.".into() }),
        Some(ip) => match ws::lan_start_via(ip, classic::LOGIN_PORT, front, classic::WORLD_PORT, ws::Layout::Classic) {
            Ok((ip, lp, _)) => (Some(format!("{ip}:{lp}")), InviteWay { id: "lan", ok: true, line: format!("Same network: on, at {ip} (friends at your home or on your network come in here first).") }),
            Err(e) => (None, InviteWay { id: "lan", ok: false, line: format!("Same network: off ({e}). Allow Leyline on private networks in your firewall, then Invite again.") }),
        },
    };
    let relay_way = if tunnel.last_error.is_empty() {
        InviteWay { id: "relay", ok: true, line: "Leyline relay: on (works from anywhere; the relay only passes the game's connection through).".into() }
    } else {
        InviteWay { id: "relay", ok: false, line: format!("Leyline relay: {}", tunnel.last_error) }
    };
    let own_way = InviteWay { id: "own", ok: false, line: "Your own address: not used for Leyline Solo.".into() };
    let link = litenet::invite_link(realms::RELAY, &mine, &secret, Some((&user, &pass)));
    let code = denhost::code_from_link(&link).unwrap_or_default();
    let link = wf::with_mark(&denways::with_ways(&link, lan.as_deref(), None));
    Ok(DenInvite { link, code, name: "your friend".into(), expires, tunnel, ways: vec![lan_way, own_way, relay_way] })
}

/// Sharing Leyline Solo ends: its same-network forwarder and its tunnel (when they are its own).
fn classic_share_off() {
    use crate::host::{litenet, worldshare as ws};
    let p = classic_paths();
    if litenet::key_path(&p.share).is_file() {
        let mine = litenet::load_or_create_key(&litenet::key_path(&p.share)).and_then(|s| litenet::public_key(&s)).map(|k| litenet::realm_id(&k)).ok();
        if mine.is_some_and(|m| litenet::state().realm_id == m) {
            litenet::stop();
        }
    }
    if ws::lan_layout() == Some(ws::Layout::Classic) {
        ws::lan_stop();
    }
    crate::host::worldfriends::front_stop();
    if let Some(w) = classic_friends_here() {
        crate::host::worldfriends::share_stopped(&w);
    }
}

#[tauri::command]
fn classic_friends() -> Vec<crate::host::classic::Friend> {
    crate::host::classic::friends(&classic_paths())
}

/// Sharing off, and every friend's login removed.
#[tauri::command]
async fn classic_share_stop() -> Result<String, String> {
    blocking(|| {
        classic_share_off();
        let n = crate::host::classic::remove_friends(&classic_paths());
        Ok(format!("Sharing stopped: friends' links don't work until you invite again ({n} friend login(s) removed)."))
    })
    .await
}

// ---- Boss Rush (host/bossrush.rs, docs/BOSS-RUSH.md; ui/bossrush.js): the exact engine (the Built-in engine's
// Boss Rush install, the real fights) or the light one (Den Lite's own recreation at the real spot).

fn bossrush_paths() -> crate::host::engine::Paths {
    crate::host::engine::Paths::for_kind(&settings::app_dir(), crate::host::engine::Kind::BossRush)
}

fn bossrush_dir() -> PathBuf {
    settings::app_dir().join("bossrush")
}

#[derive(Serialize)]
struct BossRushFight {
    key: &'static str,
    title: &'static str,
    raid: &'static str,
    map: u32,
    heroic: bool,
    light: bool,
    blurb: &'static str,
}

#[derive(Serialize)]
struct BossRushClass {
    word: &'static str,
    label: &'static str,
    specs: Vec<(&'static str, &'static str, &'static str)>,
}

#[derive(Serialize)]
struct BossRushCatalog {
    fights: Vec<BossRushFight>,
    /// Start anywhere's tree: raids, their wings and bosses, the whole raid (ui/bossstart.js).
    raids: Vec<BossRushRaid>,
    classes: Vec<BossRushClass>,
    maps: Vec<u32>,
    exact: crate::host::engine::EngineStatus,
    light: bool,
}

/// The tile's catalog, the exact engine's state and whether the light one can run here.
#[tauri::command]
async fn bossrush_catalog() -> Result<BossRushCatalog, String> {
    use crate::host::bossrush as br;
    use crate::host::lite::DenLite;
    blocking(|| {
        Ok(BossRushCatalog {
            fights: br::FIGHTS.iter().map(|f| BossRushFight { key: f.key, title: f.title, raid: f.raid, map: f.map, heroic: f.heroic, light: f.light, blurb: f.blurb }).collect(),
            raids: bossrush_raids(),
            classes: br::CLASSES.iter().map(|c| BossRushClass { word: c.0, label: c.1, specs: c.2.to_vec() }).collect(),
            maps: br::maps(),
            exact: crate::host::engine::status(&bossrush_paths()),
            light: {
                let l = crate::host::lite::Binary::find(&settings::app_dir());
                l.available() || crate::host::lite::asset().is_some()
            },
        })
    })
    .await
}

/// Set the exact engine up for Boss Rush (whatever isn't done: the engine, the raid maps from this game folder, the
/// database, the login) and start it.
#[tauri::command]
fn bossrush_setup(core: State<Core>, client: String) -> Result<String, String> {
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Boss Rush plays with a different game folder: pick one of its version.".into());
    }
    crate::host::engine::begin_kind(&settings::app_dir(), crate::host::engine::Kind::BossRush, Some(profile.path), false)
}

#[tauri::command]
fn bossrush_start() -> Result<String, String> {
    crate::host::engine::begin_kind(&settings::app_dir(), crate::host::engine::Kind::BossRush, None, false)
}

#[tauri::command]
async fn bossrush_stop() -> Result<String, String> {
    blocking(|| crate::host::engine::stop(&bossrush_paths())).await
}

#[tauri::command]
async fn bossrush_logs(lines: Option<usize>) -> Result<String, String> {
    blocking(move || Ok(crate::host::engine::logs(&bossrush_paths(), lines.unwrap_or(200).clamp(20, 2000)))).await
}

#[derive(Serialize)]
struct BossRushPlay {
    message: String,
    engine: String,
    hero: String,
    made: bool,
    /// Signed in by itself (our login screen code), else the login to type.
    auto: bool,
    account: String,
    password: String,
    code: String,
}

/// The fight picked on the tile, readied and played. Exact: the hero account's login set (once), the module asked
/// for the hero and the fight (`bossfight hero go`), then the game started signed in to the hero account, whose
/// login lands on the fight's platform. Light: the fight written for Den Lite (`<dens>/bossrush.json`), then Your
/// Den's own one-click start (the page calls den_play_lite after this).
#[tauri::command]
async fn bossrush_play(core: State<'_, Core>, client: String, fight: String, diff: String, class: String, spec: String, engine: String) -> Result<BossRushPlay, String> {
    use crate::host::bossrush as br;
    let pick = br::check(&fight, &diff, &class, &spec, &engine)?;
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Boss Rush plays with a different game folder: pick one of its version.".into());
    }
    let code = br::share_code(&pick);
    if pick.engine == br::Engine::Light {
        let p = pick.clone();
        blocking(move || {
            use crate::host::lite::DenLite;
            let dens = crate::host::lite::Binary::find(&settings::app_dir()).den_dir();
            std::fs::create_dir_all(&dens).map_err(|e| e.to_string())?;
            let id = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
            let doc = serde_json::json!({ "fight": p.fight, "diff": p.diff(), "class": p.class, "spec": p.spec, "id": id });
            settings::write_atomic(&dens.join("bossrush.json"), doc.to_string().as_bytes()).map_err(|e| e.to_string())
        })
        .await?;
        return Ok(BossRushPlay { message: "Your fight is ready: Den Lite opens it.".into(), engine: "light".into(), hero: String::new(), made: false, auto: true, account: String::new(), password: String::new(), code });
    }
    let pk = pick.clone();
    bossrush_exact(&core, &client, profile, move |main| br::go_line(main, &pk), code).await
}

/// The exact engine's one click, for a fight (bossrush_play) or a start anywhere (bossrush_start_at): the hero
/// account's login set (once), the module asked for the hero and its waiting go (`bossfight hero go ...`, the line
/// `go` makes from the player's own account), then the game started signed in to the hero account, whose login lands
/// at the start.
async fn bossrush_exact(core: &Core, client: &str, profile: ClientProfile, go: impl FnOnce(&str) -> String + Send + 'static, code: String) -> Result<BossRushPlay, String> {
    use crate::host::{bossrush as br, engine as eng};
    let p = bossrush_paths();
    let st = eng::status(&p);
    if !st.set_up {
        return Err("Set Boss Rush up first: press Set up on its page.".into());
    }
    if !st.running {
        return Err("Boss Rush isn't running: press Start first.".into());
    }
    // the Leyline addons (the BossFights window) up to date first, as every local world's Play does
    addons_before_launch(core, &profile.id, false).await;
    let (reply, hero_acc, pass) = blocking(move || {
        let p = bossrush_paths();
        let main = eng::load_state(&p).account;
        let hero_acc = br::hero_account(&main);
        let pass = eng::hero_password(&p)?;
        let [create, set] = br::account_lines(&hero_acc, &pass)?;
        let _ = eng::console(&p, &create); // there already: fine
        eng::console(&p, &set).map_err(|e| format!("Boss Rush's realm didn't take the hero's login ({e}). Is it still starting? Try again in a minute."))?;
        let text = eng::console(&p, &go(&main)).map_err(|e| {
            if e.to_ascii_lowercase().contains("no such command") {
                "Boss Rush's engine doesn't have boss fights yet: update the engine on this page.".to_string()
            } else {
                format!("Boss Rush's realm didn't answer ({e}).")
            }
        })?;
        Ok((br::parse_reply(&text), hero_acc, pass))
    })
    .await?;
    let (hero, made) = match reply {
        br::HeroReply::Ok { name, made, .. } => (name, made),
        br::HeroReply::No(why) => return Err(format!("The fight couldn't start: {why}.")),
        br::HeroReply::Nothing => return Err("Boss Rush's realm gave no answer. Try again in a moment.".into()),
    };
    let realm = eng::Kind::BossRush.realm_name();
    let _ = crate::herokey::set_login(&profile.path, &hero_acc, Some(realm));
    let refused = crate::hybrid::refused(&profile.path);
    let session = if refused { None } else { Some(crate::hybrid::Session { user: hero_acc.clone(), pass: pass.clone(), realm: Some(realm.into()), enter: true, pick: None, make: None }) };
    let (message, hybrid) = point_and_play_with(core, client, eng::REALM, true, session, true)?;
    let auto = hybrid.is_ok();
    // the world stops (everyone saved) a minute after this game closes (Settings › world_stop_on_exit), as the other
    // local worlds do
    let plays = WORLD_PLAYS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    stop_world_when_closed(core.dir.clone(), eng::Kind::BossRush, LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed), plays);
    Ok(BossRushPlay { message, engine: "exact".into(), hero, made, auto, account: if auto { String::new() } else { hero_acc }, password: if auto { String::new() } else { pass }, code })
}

// ---- Boss Rush: start anywhere (host/bossrush.rs RAIDS, the module's BossfightsStarts.h; ui/bossstart.js) -------------

#[derive(Serialize)]
struct BossRushWing {
    key: &'static str,
    title: &'static str,
    fights: Vec<&'static str>,
}

#[derive(Serialize)]
struct BossRushRaid {
    key: &'static str,
    title: &'static str,
    map: u32,
    heroic: bool,
    whole: &'static str,
    fights: Vec<&'static str>,
    wings: Vec<BossRushWing>,
}

fn bossrush_raids() -> Vec<BossRushRaid> {
    crate::host::bossrush::RAIDS
        .iter()
        .map(|r| BossRushRaid {
            key: r.key,
            title: r.title,
            map: r.map,
            heroic: r.heroic,
            whole: r.whole,
            fights: r.fights.to_vec(),
            wings: r.wings.iter().map(|w| BossRushWing { key: w.key, title: w.title, fights: w.fights.to_vec() }).collect(),
        })
        .collect()
}

/// Start anywhere: in front of a boss, at a wing's first boss, or at the raid's gate; 10 or 25; with companions (the
/// empty seats filled now) or with friends (a raid that waits for them: bossrush_go_now starts it, companions in the
/// seats left unless `companions` is off). Exact engine only (the real raid).
#[tauri::command]
async fn bossrush_start_at(core: State<'_, Core>, client: String, start: String, diff: String, size: u8, party: String, companions: bool, class: String, spec: String) -> Result<BossRushPlay, String> {
    use crate::host::bossrush as br;
    let pick = br::check_start(&start, &diff, size, &party, companions, &class, &spec)?;
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Boss Rush plays with a different game folder: pick one of its version.".into());
    }
    let pk = pick.clone();
    bossrush_exact(&core, &client, profile, move |main| br::start_go_line(main, &pk), String::new()).await
}

/// The console's answer to `bossfight rush <verb> RUSH [arg]` as the page reads it.
fn rush_json(r: &crate::host::bossrush::Rush) -> serde_json::Value {
    use crate::host::bossrush::Rush;
    match r {
        Rush::Run(x) => serde_json::json!({
            "kind": "run", "start": x.start, "diff": x.diff, "phase": x.phase, "leader": x.leader, "map": x.map, "instance": x.instance,
            "x": x.x, "y": x.y, "z": x.z, "dist": x.dist, "members": x.members, "bots": x.bots, "inside": x.inside, "step": x.step,
            "step_title": crate::host::bossrush::start(&x.step).map(|s| s.1).unwrap_or(""), "done": x.done, "of": x.of, "boss": x.boss,
            "wipes": x.wipes, "clears": x.clears, "marks": x.marks, "last": x.last, "ms": x.ms, "clock": crate::host::bossrush::clock(x.ms),
        }),
        Rush::Lobby { start, diff, waiting, size } => serde_json::json!({ "kind": "lobby", "start": start, "diff": diff, "waiting": waiting, "size": size }),
        Rush::Offline => serde_json::json!({ "kind": "offline" }),
        Rush::Ok { word } => serde_json::json!({ "kind": "ok", "word": word }),
        Rush::No { why } => serde_json::json!({ "kind": "no", "why": why }),
        Rush::Nothing => serde_json::json!({ "kind": "nothing" }),
    }
}

/// One Boss Rush console word for the player's hero (SOAP, 127.0.0.1 only), read back.
fn bossrush_rush_now(verb: &str, arg: Option<&str>) -> Result<crate::host::bossrush::Rush, String> {
    use crate::host::{bossrush as br, engine as eng};
    let p = bossrush_paths();
    if !eng::status(&p).running {
        return Err("Boss Rush isn't running.".into());
    }
    let main = eng::load_state(&p).account;
    let text = eng::console(&p, &br::rush_line(verb, &main, arg)?).map_err(|e| {
        if e.to_ascii_lowercase().contains("no such command") {
            "Boss Rush's engine is older than start anywhere: update the engine on this page.".to_string()
        } else {
            format!("Boss Rush's realm didn't answer ({e}).")
        }
    })?;
    Ok(br::parse_rush(&text))
}

/// Where the hero's raid is: the start it stands at, the phase, the party, the bosses down, the last result.
#[tauri::command]
async fn bossrush_state() -> Result<serde_json::Value, String> {
    blocking(|| bossrush_rush_now("status", None).map(|r| rush_json(&r))).await
}

/// Retry from this boss: after a wipe the raid stands again in front of the first boss still standing; after a
/// clear the same start again in a fresh instance.
#[tauri::command]
async fn bossrush_retry() -> Result<serde_json::Value, String> {
    blocking(|| bossrush_rush_now("retry", None).map(|r| rush_json(&r))).await
}

/// With friends: start now with whoever is there (companions in the seats left, unless `companions` is off).
#[tauri::command]
async fn bossrush_go_now(companions: bool) -> Result<serde_json::Value, String> {
    blocking(move || bossrush_rush_now("go", if companions { None } else { Some("nobots") }).map(|r| rush_json(&r))).await
}

/// Out of the run (the hero stays at its start).
#[tauri::command]
async fn bossrush_leave() -> Result<serde_json::Value, String> {
    blocking(|| bossrush_rush_now("leave", None).map(|r| rush_json(&r))).await
}

/// What a friend who joins Boss Rush plays: the start, the difficulty, the size, the party word and the friend's
/// hero (`<share>/bossrush-friend.json`, read when the friend's claim comes in: world_friends_of's login_for).
fn bossrush_friend_file() -> PathBuf {
    crate::host::worldshare::dir(&bossrush_paths().root).join("bossrush-friend.json")
}

/// Invite a friend into the Boss Rush raid: the same invite link as a world's (the relay tunnel, the same network),
/// and when the friend's launcher comes in, a hero made for them on the spot (the class and spec picked here; they
/// can respec in game for free) that waits at the same start, in the same raid.
#[tauri::command]
async fn bossrush_invite(start: String, diff: String, size: u8, companions: bool, friend_class: String, friend_spec: String) -> Result<DenInvite, String> {
    use crate::host::bossrush as br;
    let pick = br::check_start(&start, &diff, size, "friends", companions, &friend_class, &friend_spec)?;
    blocking(move || {
        let doc = serde_json::json!({ "start": pick.start, "diff": pick.diff_token(), "mode": pick.party.mode(), "class": pick.class, "spec": pick.spec });
        let f = bossrush_friend_file();
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        settings::write_atomic(&f, doc.to_string().as_bytes()).map_err(|e| e.to_string())?;
        world_invite_now(crate::host::engine::Kind::BossRush)
    })
    .await
}

/// A friend's claim on Boss Rush, after their login is ready: their hero (made on their hero account with the claim's
/// password, waiting at the host's start) is the login their game uses.
fn bossrush_friend_login(user: &str, pass: &str) -> Result<String, String> {
    use crate::host::{bossrush as br, engine as eng};
    let p = bossrush_paths();
    let pick: serde_json::Value = std::fs::read_to_string(bossrush_friend_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or("The host hasn't picked a fight for friends yet.")?;
    let g = |k: &str| pick.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let (diff, size) = match g("diff").as_str() {
        "normal25" => ("normal", 25),
        "heroic25" => ("heroic", 25),
        "heroic" => ("heroic", 10),
        _ => ("normal", 10),
    };
    let companions = g("mode") != "group-only";
    let sp = br::check_start(&g("start"), diff, size, "friends", companions, &g("class"), &g("spec"))?;
    let hero = br::hero_account_of(user).ok_or("That login doesn't look right.")?;
    let [create, set] = br::account_lines(&hero, pass)?;
    let _ = eng::console(&p, &create);
    eng::console(&p, &set)?;
    match br::parse_reply(&eng::console(&p, &br::start_go_line(user, &sp))?) {
        br::HeroReply::Ok { .. } => Ok(hero),
        br::HeroReply::No(why) => Err(format!("Your friend's hero couldn't be made: {why}.")),
        br::HeroReply::Nothing => Err("Boss Rush's realm gave no answer for your friend's hero.".into()),
    }
}

#[derive(Serialize)]
struct BossRushBoard {
    bests: Vec<BossRushRun>,
    recent: Vec<BossRushRun>,
}

#[derive(Serialize)]
struct BossRushRun {
    fight: String,
    title: String,
    difficulty: String,
    engine: String,
    class: String,
    spec: String,
    ms: u64,
    clock: String,
    cleared: bool,
    at: u64,
    new_best: bool,
}

fn board_view(runs: &[crate::host::bossrush::Run]) -> BossRushBoard {
    use crate::host::bossrush as br;
    let view = |r: &br::Run, new_best: bool| BossRushRun {
        fight: r.fight.clone(),
        title: br::fight(&r.fight).map(|f| f.title).unwrap_or("").into(),
        difficulty: if r.heroic { "heroic" } else { "normal" }.into(),
        engine: r.engine.word().into(),
        class: r.class.clone(),
        spec: r.spec.clone(),
        ms: r.ms,
        clock: br::clock(r.ms),
        cleared: r.cleared,
        at: r.at,
        new_best,
    };
    let bests = br::bests(runs).iter().map(|r| view(r, false)).collect();
    let recent = runs.iter().rev().take(10).map(|r| view(r, br::is_new_best(runs, r))).collect();
    BossRushBoard { bests, recent }
}

/// The local board (best times, the last runs): the exact engine's new attempts read from its own record first
/// (the module's leyline_bossfight_runs), when it is running.
#[tauri::command]
async fn bossrush_board() -> Result<BossRushBoard, String> {
    use crate::host::bossrush as br;
    blocking(|| {
        let dir = bossrush_dir();
        let file = dir.join("board.tsv");
        let last_file = dir.join("exact-last-id");
        let mut text = std::fs::read_to_string(&file).unwrap_or_default();
        // the light version's runs: Den Lite appends board lines to <dens>/bossrush-results.tsv (labs/den-lite
        // bossrush.rs RESULTS); the lines already taken are counted in light-taken
        {
            use crate::host::lite::DenLite;
            let dens = crate::host::lite::Binary::find(&settings::app_dir()).den_dir();
            let light = std::fs::read_to_string(dens.join("bossrush-results.tsv")).unwrap_or_default();
            let taken_file = dir.join("light-taken");
            let taken: usize = std::fs::read_to_string(&taken_file).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            let lines: Vec<&str> = light.lines().collect();
            let taken = if taken > lines.len() { 0 } else { taken };   // the file was started again
            if lines.len() > taken {
                for r in br::read_board(&lines[taken..].join("\n")).into_iter().filter(|r| r.engine == br::Engine::Light) {
                    text = br::add_run(&text, &r);
                }
                let _ = std::fs::create_dir_all(&dir);
                let _ = settings::write_atomic(&file, text.as_bytes());
                let _ = settings::write_atomic(&taken_file, lines.len().to_string().as_bytes());
            }
        }
        let p = bossrush_paths();
        if crate::host::engine::status(&p).running {
            let after: u64 = std::fs::read_to_string(&last_file).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            if let Ok(rows) = crate::host::engine::query(&p, "acore_characters", &br::runs_sql(after)) {
                let new = br::parse_db_runs(&rows);
                if let Some(last) = new.last().map(|x| x.0) {
                    for (_, r) in &new {
                        text = br::add_run(&text, r);
                    }
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = settings::write_atomic(&file, text.as_bytes());
                    let _ = settings::write_atomic(&last_file, last.to_string().as_bytes());
                }
            }
        }
        Ok(board_view(&br::read_board(&text)))
    })
    .await
}

/// A run the light engine finished (Den Lite writes it into the Dens folder; the page passes it on), kept on the
/// board: whether it is a new best.
#[tauri::command]
async fn bossrush_record(fight: String, diff: String, class: String, spec: String, ms: u64, cleared: bool) -> Result<bool, String> {
    use crate::host::bossrush as br;
    blocking(move || {
        let p = br::check(&fight, &diff, &class, &spec, "light")?;
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let r = br::Run { fight: p.fight, heroic: p.heroic, engine: br::Engine::Light, class: p.class, spec: p.spec, ms: ms.min(24 * 3_600_000), cleared, at };
        let dir = bossrush_dir();
        let file = dir.join("board.tsv");
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let best = br::is_new_best(&br::read_board(&text), &r);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        settings::write_atomic(&file, br::add_run(&text, &r).as_bytes()).map_err(|e| e.to_string())?;
        Ok(best)
    })
    .await
}

/// The Leaderboards view (ui/boards.js): the realms' signed boards from the website, every signature checked here
/// (boards.rs), and the separate local-run board; the last view this PC saw when the site can't be reached.
#[tauri::command]
async fn boards_get() -> Result<serde_json::Value, String> {
    blocking(|| Ok(crate::boards::load(&settings::app_dir()))).await
}

/// Share a run from this PC on the website's local-run board (only when the player presses Share): signed with this
/// launcher's own key, sent as the signed-in player. It never goes on a realm's signed board.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn boards_share_local(fight: String, diff: String, engine: String, name: String, class: String, spec: String, ms: u64, at: u64) -> Result<String, String> {
    blocking(move || {
        let me = crate::identity::peer_identity()?;
        let body = crate::boards::local_record(&me, &fight, &diff, &engine, &name, &class, &spec, ms, at)?;
        let url = format!("{}{}", crate::boards::site(), crate::boards::LOCAL_PATH);
        let reply = crate::social::post_as_player(&settings::app_dir(), &url, &body)?;
        Ok(reply.get("message").and_then(serde_json::Value::as_str).unwrap_or("Shared on the local-run board.").chars().take(200).collect())
    })
    .await
}

/// A fight code a friend shared, read back as a pick (nothing in it runs).
#[tauri::command]
fn bossrush_code(code: String) -> Result<serde_json::Value, String> {
    let p = crate::host::bossrush::read_code(&code)?;
    Ok(serde_json::json!({ "fight": p.fight, "diff": p.diff(), "class": p.class, "spec": p.spec, "engine": p.engine.word() }))
}

/// Forge, the Den's terrain editor (host/forge.rs): installed or not, open or not, and what its last close did.
#[tauri::command]
fn forge_state() -> crate::host::forge::ForgeState {
    crate::host::forge::state(&crate::settings::app_dir())
}

/// Get Forge: its release zip, checked against its sha256, unpacked into <app dir>/forge.
#[tauri::command]
async fn forge_install() -> Result<String, String> {
    blocking(|| crate::host::forge::install(&crate::settings::app_dir())).await
}

/// Open in Forge: the Den's tile from the player's own files plus its ground, Forge started on it (Den mode). When
/// Forge closes, its work becomes the Den's ground, patch-T is made again and the Docker Den's tile re-staged.
#[tauri::command]
async fn forge_open(core: State<'_, Core>, client: String) -> Result<String, String> {
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Your Den's ground is made from the game folder Your Den plays with: pick that one.".into());
    }
    blocking(move || {
        let k = crate::p2p::cmds::kit_opt();
        let (docker, _) = crate::p2p::cmds::docker_den(k.as_deref());
        let kit = if docker { k } else { None };
        crate::host::forge::open(&crate::settings::app_dir(), &profile.path, &crate::p2p::cmds::den_root(), kit)
    })
    .await
}

/// "Back to the official realm": point the client at it again.
#[tauri::command]
fn back_to_official(core: State<Core>, client: String) -> Result<String, String> {
    point_and_play(&core, &client, OFFICIAL_REALM, false)
}

/// Put back the realmlist the player had before Leyline.
#[tauri::command]
fn restore_realmlist(core: State<Core>, client: String) -> Result<String, String> {
    let p = core.client(&client)?;
    if !clients::restore_realmlist(&p).map_err(|e| e.to_string())? {
        return Ok("Leyline never changed this client's realmlist, so there is nothing to put back.".into());
    }
    let now = clients::read_realmlist(&p).unwrap_or_default();
    core.update(|s| {
        if let Some(c) = s.client_mut(&client) {
            c.realmlist = now.clone();
        }
        Ok(())
    })?;
    Ok(format!("Put back your own realmlist ({}).", if now.is_empty() { "empty" } else { &now }))
}

#[tauri::command]
async fn realm_listing(core: State<'_, Core>) -> Result<Listing, String> {
    let dir = core.dir.clone();
    blocking(move || realms::fetch(&dir)).await
}

#[tauri::command]
async fn realms_list(core: State<'_, Core>) -> Result<RealmList, String> {
    let dir = core.dir.clone();
    let l = blocking(move || Ok(realms::fetch_all(&dir))).await?;
    core.allow(l.realms.iter().flat_map(|r| [r.join_url.clone(), r.discord.clone(), r.report_url.clone()]).flatten());
    Ok(l)
}

/// The realm list as last fetched, at once and without the network (None before the first fetch): the page shows it
/// while `realms_list` asks the network.
#[tauri::command]
async fn realms_list_cached(core: State<'_, Core>) -> Result<Option<RealmList>, String> {
    let dir = core.dir.clone();
    let l = blocking(move || Ok(realms::cached_list(&dir))).await?;
    if let Some(l) = &l {
        core.allow(l.realms.iter().flat_map(|r| [r.join_url.clone(), r.discord.clone(), r.report_url.clone()]).flatten());
    }
    Ok(l)
}

/// Open the browser client. Only that one address: the UI cannot open arbitrary URLs through this.
#[tauri::command]
fn open_browser_play(app: AppHandle) -> Result<(), String> {
    app.opener().open_url(BROWSER_PLAY_URL, None::<&str>).map_err(|e| e.to_string())
}

/// Ping the realm's login port and read its status file: the official one, or a relay realm's own.
#[tauri::command]
async fn realm_status(realm: String, status_url: Option<String>) -> Result<status::RealmStatus, String> {
    let realm = realm.trim().to_string();
    if !clients::valid_realm_address(&realm) {
        return Err("That realm address does not look right.".into());
    }
    let url = match status_url {
        Some(u) if realms::status_url_ok(&u) => Some(u),
        Some(_) => return Err("Not a realm status address.".into()),
        None => crate::addresses::is_official_realm(&status::host_port(&realm).0).then(|| status::OFFICIAL_STATUS_URL.to_string()),
    };
    blocking(move || Ok(status::check(&realm, url.as_deref()))).await
}

/// Open one of the launcher's fixed pages (`crate::LINKS`) by name: Discord, news, docs, the site.
#[tauri::command]
fn open_link(app: AppHandle, name: String) -> Result<(), String> {
    let url = crate::link(&name).ok_or("Unknown link.")?;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// The website's host form filled in with the choices a `leyline://host?` link brought (deeplink.rs): checked again
/// here, so the UI can only open the site's own host page.
#[tauri::command]
fn open_host_form(app: AppHandle, query: String) -> Result<(), String> {
    let q = crate::deeplink::host_query(&query).filter(|q| !q.starts_with("manage="))
        .ok_or("Those choices are not valid: set them up again in the game.")?;
    let url = format!("{}?{q}", crate::host::HOST_PAGE);
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Open an address the core handed the UI (a mod's repo or page, an author, a realm's join page or Discord,
/// the release page). Anything else is refused.
#[tauri::command]
fn open_url(app: AppHandle, core: State<Core>, url: String) -> Result<(), String> {
    let ok = core.known_urls.lock().map(|k| k.contains(&url)).unwrap_or(false);
    if !ok || !net::plain_https(&url) {
        return Err("Leyline only opens the pages it listed itself.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------------ mods

#[tauri::command]
async fn mods_catalog(core: State<'_, Core>) -> Result<Catalog, String> {
    let dir = core.dir.clone();
    let c = blocking(move || catalog::fetch(&dir)).await?;
    core.allow(c.projects.iter().flat_map(|p| [p.repo.clone(), p.page.clone(), p.author_url.clone()]).filter(|u| !u.is_empty()));
    Ok(c)
}

/// The mods list as last fetched, at once and without the network (None before the first fetch): Discover shows it
/// while `mods_catalog` asks the network.
#[tauri::command]
async fn mods_catalog_cached(core: State<'_, Core>) -> Result<Option<Catalog>, String> {
    let dir = core.dir.clone();
    let c = blocking(move || Ok(catalog::cached_catalog(&dir, catalog::URL))).await?;
    if let Some(c) = &c {
        core.allow(c.projects.iter().flat_map(|p| [p.repo.clone(), p.page.clone(), p.author_url.clone()]).filter(|u| !u.is_empty()));
    }
    Ok(c)
}

/// The catalog count (L0's read-only listing shape), for the side bar.
#[tauri::command]
async fn catalog_listing(core: State<'_, Core>) -> Result<Listing, String> {
    let c = mods_catalog(core).await?;
    Ok(Listing {
        count: c.projects.len(),
        names: c.projects.iter().take(100).map(|p| p.name.clone()).collect(),
        from_cache: c.from_cache,
        saved_at: c.saved_at,
        error: c.error,
    })
}

#[derive(Serialize)]
struct ClientMods {
    client: String,
    build: clients::Build,
    mods: Vec<InstalledView>,
}

#[tauri::command]
fn mods_installed(core: State<Core>, client: String) -> Result<ClientMods, String> {
    let p = core.client(&client)?;
    let man = mods::load(&p.path);
    let cat = catalog::cached(&core.dir, catalog::URL).unwrap_or_default();
    Ok(ClientMods { client, build: p.build, mods: mods::with_updates(&man, &cat, p.build) })
}

fn with_lock<T>(core: &Core, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
    f()
}

/// Install (or update) a catalog mod into a client. The project is looked up in the cached catalog by id.
#[tauri::command]
async fn mod_install(core: State<'_, Core>, client: String, project: String) -> Result<Installed, String> {
    let p = core.client(&client)?;
    let cat = catalog::cached(&core.dir, catalog::URL).ok_or("Open Discover first so Leyline has the catalog.")?;
    let proj = cat.into_iter().find(|x| x.id == project).ok_or("That mod is not in the catalog any more.")?;
    let origin = net::origin(catalog::URL).unwrap_or_default();
    let lock = core.mods_lock.clone();
    let scratch = core.dir.clone();
    let ok_keys = core.snapshot().native_ok;
    let (count, build, id) = (core.snapshot().count_installs, p.build, proj.id.clone());
    let done = blocking(move || {
        let _g = lock.lock().map_err(|_| "mod lock poisoned")?;
        mods::install_project(&p.path, p.build, &proj, &origin, &scratch, &|k: &str| ok_keys.iter().any(|x| x == k))
    })
    .await;
    if done.is_ok() && count {
        // The anonymous install count (rankings.rs): best effort, never in the way of the install.
        std::thread::spawn(move || {
            let _ = crate::rankings::ping(&crate::rankings::site(), &id, build);
        });
    }
    done
}

// ------------------------------------------------------------------------------------------------ rankings

/// Discover's order: the catalog's project ids sorted and filtered (rankings.rs), with the numbers to show.
/// `refresh` fetches mods.json again; otherwise the cached copy is used when there is one.
#[tauri::command]
async fn mods_ranked(
    core: State<'_, Core>,
    sort: String,
    kind: String,
    client: Option<String>,
    refresh: bool,
) -> Result<crate::rankings::Ranked, String> {
    let dir = core.dir.clone();
    let build = client.as_deref().and_then(|id| core.snapshot().client(id).map(|c| c.build));
    let kind = if matches!(kind.as_str(), "addon" | "mod" | "mode" | "theme") { kind } else { String::new() };
    let (metrics, from_cache, error) = blocking(move || {
        let have = crate::rankings::cached(&dir);
        Ok(if refresh || have.is_empty() { crate::rankings::fetch(&dir, &crate::rankings::site()) } else { (have, true, None) })
    })
    .await?;
    let cat = catalog::cached(&core.dir, catalog::URL).unwrap_or_default();
    let ids = crate::rankings::rank(&cat, &metrics, crate::rankings::Sort::parse(&sort), &kind, build);
    Ok(crate::rankings::Ranked { ids, metrics, from_cache, error })
}

/// Rate a catalog project 1-5 (0 takes it back), signed in with the Leyline sign-in (Home's profile). "signed_out" otherwise.
#[tauri::command]
async fn mod_rate(core: State<'_, Core>, project: String, stars: u8) -> Result<serde_json::Value, String> {
    let dir = core.dir.clone();
    blocking(move || crate::rankings::rate(&dir, &crate::rankings::site(), &project, stars)).await
}

/// The player's own rating of a project (0: none or not signed in).
#[tauri::command]
async fn mod_my_rating(core: State<'_, Core>, project: String) -> Result<u8, String> {
    let dir = core.dir.clone();
    blocking(move || Ok(crate::rankings::mine(&dir, &crate::rankings::site(), &project))).await
}

/// Settings › Count my installs.
#[tauri::command]
fn set_count_installs(core: State<Core>, on: bool) -> Result<(), String> {
    core.update(|s| {
        s.count_installs = on;
        Ok(())
    })
}

/// The `leyline://` link waiting for the UI, once (then None): the one Leyline started with, or one that reached the
/// open launcher (`deep_link_arrived`).
#[tauri::command]
fn deep_link_take(core: State<Core>) -> Option<crate::deeplink::Link> {
    core.deep_link.lock().ok().and_then(|mut d| d.take())
}

/// A `leyline://` link reached the open launcher (a second start's arguments on Windows and Linux, the deep-link
/// plugin on macOS): keep it for `deep_link_take`, bring the window forward and tell the UI (ui/app.js listens for
/// `deep-link`). Nothing starts here: the UI shows the page and the player clicks.
fn deep_link_arrived(app: &AppHandle, link: Option<crate::deeplink::Link>) {
    use tauri::Emitter;
    let Some(link) = link else { return };
    let fresh = app.state::<Core>().deep_link.lock().map(|mut d| d.put(link, crate::denclick::now_ms())).unwrap_or(false);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    if fresh {
        let _ = app.emit_to("main", "deep-link", ());
    }
}

/// Install a dropped or picked .zip, .MPQ, native file or folder.
#[tauri::command]
fn mod_install_local(core: State<Core>, client: String, path: String) -> Result<Installed, String> {
    let p = core.client(&client)?;
    let src = PathBuf::from(path.trim());
    if src.starts_with(&p.path) {
        return Err("That is already inside this client's folder: drop the zip or folder from somewhere else.".into());
    }
    let ok_keys = core.snapshot().native_ok;
    with_lock(&core, || mods::install_local(&p.path, p.build, &src, &|k: &str| ok_keys.iter().any(|x| x == k)))
}

/// The player read the native-code notice for this mod and said yes: remembered, so it is asked only once.
/// Nothing is installed or run here; the UI installs again afterwards.
#[tauri::command]
fn mod_native_confirm(core: State<Core>, key: String) -> Result<(), String> {
    let ok = !key.is_empty() && key.len() <= 80 && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-'));
    if !ok {
        return Err("That is not a mod.".into());
    }
    core.update(|s| {
        if !s.native_allowed(&key) {
            s.native_ok.push(key);
        }
        Ok(())
    })
}

/// The launcher's guide (ui/guide.js): the first-run tour or a one-time encouragement line was shown, so it never
/// shows again. Only a short key is kept (Settings::seen).
#[tauri::command]
fn guide_seen(core: State<Core>, key: String) -> Result<(), String> {
    core.update(|s| if s.mark_seen(&key) { Ok(()) } else { Err("That is not a guide step.".into()) })
}

#[derive(Serialize)]
struct ModUpdate {
    client: String,
    client_name: String,
    key: String,
    name: String,
    version: String,
    update: String,
}

/// Every installed Workshop mod, in every client, that the cached catalog has a newer version of (the title
/// bar's update pill and UPDATE ALL).
#[tauri::command]
fn mods_updates(core: State<Core>) -> Vec<ModUpdate> {
    let cat = catalog::cached(&core.dir, catalog::URL).unwrap_or_default();
    let mut out = Vec::new();
    for c in core.snapshot().clients {
        let man = mods::load(&c.path);
        for v in mods::with_updates(&man, &cat, c.build) {
            if let Some(update) = v.update {
                out.push(ModUpdate {
                    client: c.id.clone(),
                    client_name: c.name.clone(),
                    key: v.mod_.key,
                    name: v.mod_.name,
                    version: v.mod_.version,
                    update,
                });
            }
        }
    }
    out
}

#[tauri::command]
fn mod_set_enabled(core: State<Core>, client: String, key: String, on: bool) -> Result<(), String> {
    let p = core.client(&client)?;
    with_lock(&core, || mods::set_enabled(&p.path, &key, on))
}

#[tauri::command]
fn mod_uninstall(core: State<Core>, client: String, key: String) -> Result<Removed, String> {
    let p = core.client(&client)?;
    with_lock(&core, || mods::uninstall(&p.path, &key))
}

#[tauri::command]
async fn pick_mod_file(app: AppHandle) -> Option<String> {
    pick(app, "Choose a mod (.zip), a client patch (.MPQ) or a native mod (.dll, .asi)", true, true).await
}

#[tauri::command]
async fn pick_mod_folder(app: AppHandle) -> Option<String> {
    pick(app, "Choose an addon folder", false, false).await
}

// ------------------------------------------------------------------------------------------------ hosting

fn kit_info(core: &Core) -> KitInfo {
    let kit = core.kit();
    host::inspect(kit.as_deref(), dirs::download_dir().as_deref())
}

#[tauri::command]
fn host_info(core: State<Core>) -> KitInfo {
    kit_info(&core)
}

#[tauri::command]
fn set_host_kit(core: State<Core>, path: String) -> Result<KitInfo, String> {
    let p = PathBuf::from(path.trim());
    if !p.is_dir() {
        return Err("That is not a folder.".into());
    }
    core.update(|s| {
        s.host_kit = p.display().to_string();
        Ok(())
    })?;
    Ok(kit_info(&core))
}

#[tauri::command]
async fn pick_host_kit(app: AppHandle) -> Option<String> {
    pick(app, "Choose your host kit folder", false, false).await
}

/// Copy a realm file found in Downloads into the kit.
#[tauri::command]
fn host_copy_seed(core: State<Core>, file_name: String) -> Result<KitInfo, String> {
    let kit = core.kit().ok_or("Choose your host kit folder first.")?;
    let info = kit_info(&core);
    let seed = info.seeds.iter().find(|s| s.file_name == file_name && !s.in_kit).ok_or("That realm file is not in Downloads any more.")?;
    host::copy_seed_into_kit(&seed.path, &kit)?;
    Ok(kit_info(&core))
}

/// Start the kit (a seed's first run, or the kit's own host script) in a window of its own.
#[tauri::command]
fn host_start(core: State<Core>, seed: Option<String>) -> Result<String, String> {
    let kit = core.kit().ok_or("Choose your host kit folder first.")?;
    let plan = host::plan(&kit, seed.as_deref(), HostOs::current(), host::on_path)?;
    host::run(&plan)?;
    Ok(plan.note)
}

/// The kit's realm as the relay sees it.
#[tauri::command]
async fn host_status(core: State<'_, Core>, realm_id: String) -> Result<HostStatus, String> {
    if !realms::realm_id_ok(&realm_id) {
        return Err("That is not a realm id.".into());
    }
    let info = kit_info(&core);
    let relay = info.realm.as_ref().filter(|r| r.realm_id == realm_id).map(|r| r.relay.clone()).unwrap_or_else(|| realms::RELAY.into());
    let listed = realms::cached(&core.dir).iter().any(|r| r.realm_id.as_deref() == Some(realm_id.as_str()));
    blocking(move || host::status(&format!("https://{relay}"), &realm_id, listed)).await
}

// ------------------------------------------------------------------------------------------------ updates

/// What the title bar and Settings show about Leyline's own updates.
#[derive(Debug, Clone, Default, Serialize)]
struct UpdView {
    /// `auto`: downloads in the background, installs on "Restart". `link`: only a link to the release page.
    mode: &'static str,
    /// Why it is `link`, in plain words.
    note: String,
    channel: Channel,
    info: Option<UpdateInfo>,
    checking: bool,
    downloading: bool,
    /// A downloaded, signature-checked version waiting for "Restart".
    ready: Option<String>,
    /// The update the last run left waiting is going in now (no Restart press): the title bar says so.
    finishing: bool,
    error: Option<String>,
    game_running: bool,
    /// Seconds since 1970 of the last check.
    checked_at: Option<u64>,
}

#[derive(Default)]
struct UpdState {
    view: UpdView,
    pending: Option<(tauri_plugin_updater::Update, Vec<u8>)>,
    /// The version the last run downloaded and left waiting (update::READY_FILE): finished by the first check.
    waiting: Option<String>,
}

/// Whether this build can update itself, and if not, why.
fn updater_mode(app: &AppHandle) -> (&'static str, String) {
    if cfg!(debug_assertions) {
        return ("link", "This is a development build: it does not update itself.".into());
    }
    let pk = app.config().plugins.0.get("updater").and_then(|u| u.get("pubkey")).and_then(|v| v.as_str()).unwrap_or("");
    if !update::pubkey_ok(pk) {
        return ("link", "Automatic updates are not turned on for this build yet: new versions are on the release page.".into());
    }
    ("auto", String::new())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn upd_view(core: &Core) -> UpdView {
    let mut v = core.upd.lock().map(|u| u.view.clone()).unwrap_or_default();
    v.game_running = core.game_running();
    v.channel = core.snapshot().update_channel;
    v
}

/// Look for a newer Leyline on the player's channel; in `auto` mode, download it in the background so it is
/// ready for "Restart". Never installs and never restarts by itself.
async fn run_update_check(app: AppHandle) -> UpdView {
    let core = app.state::<Core>();
    let (mode, note) = updater_mode(&app);
    let channel = core.snapshot().update_channel;
    if let Ok(mut u) = core.upd.lock() {
        if u.view.checking || u.view.downloading {
            drop(u);
            return upd_view(&core);
        }
        u.view.checking = true;
        u.view.mode = mode;
        u.view.note = note;
    }
    let dir = core.dir.clone();
    let info = blocking(move || Ok(update::check(&dir, env!("CARGO_PKG_VERSION"), channel))).await.unwrap_or_else(|e| UpdateInfo {
        current: env!("CARGO_PKG_VERSION").into(),
        channel,
        error: Some(e),
        ..Default::default()
    });
    core.allow(info.url.clone());
    // Every source that offers a newer Leyline, in order: GitHub's release, our own mirror, the mirrors the signed
    // release log names (update::sources; docs/RESILIENCE.md). Each manifest is checked here (downloads only from
    // that source, never a version at or below this one); each download is checked by the updater plugin against
    // the public key in tauri.conf.json, so a mirror can be down or wrong but can't push a bad build.
    let (cands, src_notes) = {
        let (dir, info) = (core.dir.clone(), info.clone());
        blocking(move || {
            let mesh_on = crate::p2p::flags::current(&dir).mesh_realms;
            let _ = crate::releaselog::refresh(&dir, mesh_on);
            // "Help share Leyline's own files" (p2p/share.rs): off unless the site offers it and this PC turned it on
            std::thread::spawn({
                let dir = dir.clone();
                move || drop(crate::p2p::share::run(&dir))
            });
            let log = crate::releaselog::trusted_key().map(|k| crate::releaselog::load(&dir, k));
            let srcs = update::sources(&info, &crate::branding::get().updates.mirrors, log.as_ref());
            Ok(update::candidates(&srcs, env!("CARGO_PKG_VERSION"), &update::this_platform(), &|u| net::get(u, net::TIMEOUT).map(String::into_bytes)))
        })
        .await
        .unwrap_or_default()
    };
    let mut info = info;
    if let (false, Some(c)) = (info.newer, cands.first()) {
        // GitHub did not answer (or had nothing): a mirror did
        info.newer = true;
        info.latest = Some(c.manifest.version.clone());
    }
    let best = cands.first().map(|c| c.manifest.version.clone());
    let want = (mode == "auto" && !cands.is_empty()).then_some(());
    let already = core.upd.lock().ok().and_then(|u| u.pending.as_ref().map(|(p, _)| p.version.clone()));
    if let Ok(mut u) = core.upd.lock() {
        u.view.checking = false;
        u.view.checked_at = Some(now_secs());
        u.view.error = info.error.clone().filter(|_| cands.is_empty()).or_else(|| (cands.is_empty() && info.newer && !src_notes.is_empty()).then(|| src_notes.join("; ")));
        u.view.downloading = want.is_some() && already != best;
        u.view.info = Some(info.clone());
    }
    if let (Some(()), true) = (want, already != best) {
        let res: Result<Option<(tauri_plugin_updater::Update, Vec<u8>)>, String> = async {
            let mut notes = Vec::new();
            for c in &cands {
                // the built-in public key first, then the owner's offline backup keys (branding.json
                // updates.backup_pubkeys), so a release signed with a backup key still installs if the main key is lost
                let mut keys: Vec<Option<String>> = vec![None];
                keys.extend(crate::branding::get().updates.backup_pubkeys.iter().cloned().map(Some));
                let mut one: Result<Option<(tauri_plugin_updater::Update, Vec<u8>)>, String> = Ok(None);
                for key in keys {
                    one = async {
                        let endpoint: tauri::Url = c.source.manifest_url.parse().map_err(|e| format!("{e}"))?;
                        let mut b = app.updater_builder();
                        if let Some(k) = key {
                            b = b.pubkey(k);
                        }
                        let updater = b.endpoints(vec![endpoint]).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
                        let Some(up) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(None) };
                        if !update::is_newer(&up.version, env!("CARGO_PKG_VERSION")) {
                            return Err("not newer: never installed".into());
                        }
                        // the plugin checks the download's signature against that public key
                        let bytes = up.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
                        Ok(Some((up, bytes)))
                    }
                    .await;
                    if !matches!(&one, Err(e) if e.to_ascii_lowercase().contains("signature")) {
                        break;
                    }
                }
                match one {
                    Ok(Some(x)) => return Ok(Some(x)),
                    Ok(None) => notes.push(format!("{}: nothing newer", c.source.name)),
                    Err(e) => notes.push(format!("{}: {e}", c.source.name)),
                }
            }
            if notes.iter().all(|n| n.ends_with("nothing newer")) {
                return Ok(None);
            }
            Err(notes.join("; "))
        }
        .await;
        if let Ok(mut u) = core.upd.lock() {
            u.view.downloading = false;
            match res {
                Ok(Some((up, bytes))) => {
                    u.view.ready = Some(up.version.clone());
                    update::write_ready(&core.dir, &up.version);
                    let waiting = u.waiting.take();
                    if update::finish_now(waiting.as_deref(), env!("CARGO_PKG_VERSION"), &up.version, core.game_running()) {
                        u.view.finishing = true;
                    }
                    u.pending = Some((up, bytes));
                }
                Ok(None) => {}
                Err(e) => u.view.error = Some(format!("Could not get the update: {e}")),
            }
        }
    }
    // The update the last run left waiting: in now, before the player starts anything ("Finishing the update…").
    if core.upd.lock().map(|u| u.view.finishing).unwrap_or(false) {
        std::thread::sleep(Duration::from_millis(2500));
        if let Err(e) = install_and_restart(&app, &core) {
            if let Ok(mut u) = core.upd.lock() {
                u.view.finishing = false;
                u.view.error = Some(e);
            }
        }
    }
    upd_view(&core)
}

/// Install the downloaded update and start the new Leyline. Refused while a game Leyline started is running.
fn install_and_restart(app: &AppHandle, core: &Core) -> Result<(), String> {
    if core.game_running() {
        return Err("Your game is still running. Close it first: Leyline restarts only when no game is open.".into());
    }
    let pending = core.upd.lock().map_err(|_| "update lock poisoned")?.pending.take();
    let Some((up, bytes)) = pending else {
        return Err("No update is ready yet.".into());
    };
    // On Windows the installer takes over and closes Leyline itself; elsewhere Leyline restarts into the new one.
    // A failed install keeps the download: Restart can try again at once (review 4 Nov: it said "No update is ready
    // yet" until the next check, 30 minutes later).
    if let Err(e) = up.install(&bytes) {
        if let Ok(mut u) = core.upd.lock() {
            if u.pending.is_none() {
                u.pending = Some((up, bytes));
            }
        }
        return Err(format!("The update could not be installed: {e}"));
    }
    app.restart()
}

#[tauri::command]
fn launcher_update_state(core: State<Core>) -> UpdView {
    upd_view(&core)
}

#[tauri::command]
async fn launcher_update_check(app: AppHandle) -> Result<UpdView, String> {
    Ok(run_update_check(app).await)
}

#[tauri::command]
async fn set_update_channel(app: AppHandle, channel: Channel) -> Result<UpdView, String> {
    {
        let core = app.state::<Core>();
        core.update(|s| {
            s.update_channel = channel;
            Ok(())
        })?;
        // A download from the other channel is not wanted any more.
        if let Ok(mut u) = core.upd.lock() {
            u.pending = None;
            u.view.ready = None;
        };
        update::clear_ready(&core.dir);
    }
    Ok(run_update_check(app).await)
}

/// "Update ready, restart": install the downloaded update and start the new Leyline. Refused while a game
/// Leyline started is running, so a game is never cut off.
#[tauri::command]
fn launcher_update_restart(app: AppHandle, core: State<Core>) -> Result<(), String> {
    install_and_restart(&app, &core)
}

/// Check at start, then every 30 minutes (update::CHECK_EVERY_SECS), for as long as Leyline runs. An update the last run downloaded and left
/// waiting is finished by the first check (update::finish_now).
fn start_update_timer(app: AppHandle) {
    {
        let core = app.state::<Core>();
        let waiting = update::take_ready_at_start(&core.dir, env!("CARGO_PKG_VERSION"));
        if let Ok(mut u) = core.upd.lock() {
            u.waiting = waiting;
        };
    }
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(run_update_check(app.clone()));
        std::thread::sleep(Duration::from_secs(update::CHECK_EVERY_SECS));
    });
}

pub fn run() {
    let dir = settings::app_dir();
    let s = settings::load(&dir);
    tauri::Builder::default()
        // One launcher: a second start (a leyline:// link clicked while Leyline is open, or a second shortcut click)
        // hands its arguments to this one and quits; this window comes forward (deep_link_arrived). First, as the
        // plugin asks.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let link = crate::deeplink::from_args(argv);
            if link.is_none() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            deep_link_arrived(app, link);
        }))
        // leyline:// links on macOS (always handed over as an event, never as an argument) and while open elsewhere
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Core {
            dir,
            settings: Mutex::new(s),
            known_urls: Mutex::new(HashSet::new()),
            mods_lock: Arc::new(Mutex::new(())),
            games: Mutex::new(Vec::new()),
            upd: Mutex::new(UpdState::default()),
            deep_link: Mutex::new(crate::deeplink::Inbox::new(crate::deeplink::from_args(std::env::args()))),
        })
        .setup(|app| {
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let h = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    deep_link_arrived(&h, crate::deeplink::from_urls(event.urls().iter().map(|u| u.as_str())));
                });
                // macOS: a link that started Leyline arrives here rather than in the arguments
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    let link = crate::deeplink::from_urls(urls.iter().map(|u| u.as_str()));
                    if let (Some(link), Ok(mut d)) = (link, app.state::<Core>().deep_link.lock()) {
                        if d.waiting.is_none() {
                            d.put(link, crate::denclick::now_ms());
                        }
                    }
                }
            }
            start_update_timer(app.handle().clone());
            crate::p2p::cmds::magnet_from_args(); // a shared magnet: link (only with the magnet_links flag on)
            crate::agents::commands::start(app.handle().clone()); // Agents: the heartbeat `Leyline mcp` looks for (agents.rs)
            // Be a node (p2p/node.rs): its loop, when this PC turned it on; started by "start with Windows" (--node):
            // the window opens minimized, the node working
            crate::p2p::node::start(settings::app_dir());
            if crate::p2p::node::started_as_node(std::env::args()) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.minimize();
                }
            }
            // Your Den shared with friends before: back on the Leyline network by itself (host/denhost.rs)
            std::thread::spawn(|| {
                if let Err(e) = den_share_resume() {
                    eprintln!("leyline: Your Den's sharing didn't come back: {e}");
                }
            });
            Ok(())
        })
        // Be a node's "keep running when the window closes": closing the main window minimizes it instead while the
        // node is on and that box is ticked (no tray icon: that needs the tauri `tray-icon` feature, a new crate in the
        // tree). Every other window, and every close with the box unticked, closes as before.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && crate::p2p::node::keep_running_on_close(&settings::app_dir()) {
                    api.prevent_close();
                    let _ = window.minimize();
                } else if window.label() == "main" && close_with_world(window) {
                    // a world is running: it is saved and stopped first, or the page asks (friends are in it)
                    api.prevent_close();
                } else if window.label() == "main" {
                    // Leyline closes: friends see "offline" now instead of ten minutes later (at most a second's wait;
                    // a slow relay mailbox is left to time out on its own)
                    if let Some(h) = crate::p2p::friendnet::current() {
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            crate::p2p::friendnet::stop(&h);
                            let _ = tx.send(());
                        });
                        let _ = rx.recv_timeout(Duration::from_millis(1200));
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            platform,
            get_state,
            pick_folder,
            detect_client,
            add_client,
            check_client,
            rename_client,
            remove_client,
            select,
            set_wine,
            play,
            play_realm,
            play_realm_herokey,
            join_invite,
            back_to_official,
            restore_realmlist,
            catalog_listing,
            realm_listing,
            realms_list,
            realms_list_cached, // the kept list at once, then realms_list (audit 12 Nov)
            mods_catalog_cached,
            open_browser_play,
            realm_status,
            open_link,
            open_url,
            open_host_form,
            mods_catalog,
            mods_installed,
            mod_install,
            mod_install_local,
            mod_set_enabled,
            mod_uninstall,
            mod_native_confirm,
            guide_seen, // the first-run tour and the one-time encouragement lines (ui/guide.js)
            mods_updates,
            mods_ranked, // Discover's sort tabs, ratings, leyline:// links (rankings.rs, deeplink.rs)
            mod_rate,
            mod_my_rating,
            set_count_installs,
            set_hybrid_login,
            set_signin_cover, // the sign-in cover (cover.rs): its switch, its page's state and "Show the game"
            cover_state,
            cover_hide,
            cover_link,
            hybrid_status,
            deep_link_take,
            pick_mod_file,
            pick_mod_folder,
            host_info,
            set_host_kit,
            pick_host_kit,
            host_copy_seed,
            host_start,
            host_status,
            launcher_update_state,
            launcher_update_check,
            launcher_update_restart,
            crate::gfx::commands::gfx_get,
            crate::gfx::commands::gfx_set,
            crate::gfx::commands::gfx_preset,
            crate::gfx::commands::gfx_restore,
            crate::looks::commands::looks_state,
            crate::looks::commands::looks_apply,
            crate::looks::commands::looks_remove,
            crate::looks::commands::looks_reset,
            crate::loadouts::commands::loadouts_status,
            crate::loadouts::commands::loadout_new,
            crate::loadouts::commands::loadout_add_shared,
            crate::loadouts::commands::loadout_import,
            crate::loadouts::commands::loadout_code,
            crate::loadouts::commands::loadout_copy,
            crate::loadouts::commands::loadout_rename,
            crate::loadouts::commands::loadout_set_gfx,
            crate::loadouts::commands::loadout_delete,
            crate::loadouts::commands::loadout_switch,
            crate::clientimport::commands::client_import_check,
            crate::clientimport::commands::client_import_start,
            crate::clientimport::commands::client_import_state,
            crate::clientimport::commands::client_import_cancel,
            crate::clientimport::commands::client_import_pause,
            crate::clientimport::commands::client_import_share,
            crate::clientimport::commands::client_seed_state,
            crate::clientimport::commands::client_seed_stop,
            crate::clientimport::commands::client_import_pick_zip,
            crate::clientimport::commands::client_import_pick_folder,
            crate::clientimport::commands::client_import_pick_dest,
            crate::clientimport::commands::testing_get,
            crate::clientimport::commands::testing_set,
            set_update_channel,
            crate::themes::commands::themes_list, crate::themes::commands::theme_apply, crate::themes::commands::theme_css, crate::themes::commands::theme_install, crate::themes::commands::theme_install_local, crate::themes::commands::theme_remove, crate::themes::commands::pick_theme, // themes.rs
            // Host pack, Docker, realm stop/logs/invite, P2P and torrent flags (src/p2p/cmds.rs)
            crate::p2p::cmds::hostpack_info,
            crate::p2p::cmds::hostpack_install,
            crate::p2p::cmds::bots_status,
            crate::p2p::cmds::bots_update,
            crate::p2p::cmds::docker_info,
            crate::p2p::cmds::docker_start, // Start Docker: Docker Desktop's own start, else its app (host/docker.rs start)
            crate::p2p::cmds::host_profiles, // host any client (host/profiles.rs)
            crate::p2p::cmds::host_choose,
            crate::p2p::cmds::host_stop,
            crate::p2p::cmds::host_logs,
            crate::p2p::cmds::host_invite,
            crate::p2p::cmds::host_new_invite,
            crate::p2p::cmds::host_listing, // the realm list details (host.rs ListingDetails)
            crate::p2p::cmds::host_volunteer_relay, // Leyline mesh: relay for others (host.rs volunteer_relay)
            crate::p2p::cmds::host_listing_set,
            // the host console: health, GM controls, Check my realm, realm tools, Link my server (host/console.rs)
            crate::p2p::cmds::host_console,
            crate::p2p::cmds::host_gm,
            crate::p2p::cmds::host_dens, // a Den's save files: list, a copy, add one, open one (host/dens.rs)
            crate::p2p::cmds::host_den_text,
            crate::p2p::cmds::host_den_add,
            crate::p2p::cmds::den_state, // Your Den: one page, Den Lite or the Docker Den (host/yourden.rs, host/lite.rs)
            crate::p2p::cmds::den_lite_stop,
            crate::p2p::cmds::den_text,
            crate::p2p::cmds::den_add,
            crate::p2p::cmds::den_open,
            crate::p2p::cmds::den_fetch,
            crate::p2p::cmds::den_share_move,
            den_play_lite,
            den_invite, // Invite friends on Den Lite: the relay tunnel and a login per friend (host/litenet.rs)
            den_friends, // Your Den's Friends list: who's in, the logins that work, sharing (host/denhost.rs)
            den_friend,  // kick, build on/off, keep for one friend (host/denhost.rs)
            den_net_state,
            den_net_stop,
            host_ready, // Ready for friends?: the checklist before an invite (host/ready.rs)
            host_ready_fix,
            realm_answers, // the friend's side: the host's world answers (host/ready.rs)
            den_own_address, // Invite: your own address for friends (host/denways.rs)
            den_models, // My models (host/denmodels.rs)
            den_models_add,
            den_models_remove,
            den_prefill_account,
            den_join_page,
            engine_status, // Single player's Built-in engine (host/engine.rs, ui/single.js)
            engine_setup,
            engine_start,
            engine_progress,
            engine_stop,
            engine_logs,
            engine_backup,
            engine_update_check,
            engine_play,
            world_status, // Single player's worlds by kind: the original world, Quick start, Characters, Invite a friend
            world_setup,
            world_start,
            world_stop,
            world_logs,
            world_backup,
            world_backups,
            world_restore,
            world_gather_chars,
            world_password,
            world_update_check,
            world_update,
            world_choices,
            world_settings_get, // Single player's world settings before the first load and later (host/spsettings.rs)
            world_settings_set,
            world_mods_get, // Single player's "Mods in this world": the engine's own modules, off or on (host/worldmods.rs)
            world_mods_set,
            world_play,
            world_find_folder, // one click (docs/SINGLE-PLAYER-ONE-CLICK.md §contract): the folder, the disk, resume, Repair
            world_preflight,
            world_resume_pending,
            world_repair,
            set_world_stop_on_exit,
            set_world_stop_on_close,
            peer_key_export,
            peer_key_import,
            close_leyline,
            local_characters,
            online_characters,
            world_invite,
            world_share_stop,
            world_friends,
            world_friend,
            friend_worlds,
            friend_world_forget,
            join_friend_again,
            friends_state,
            friends_start,
            friends_stop,
            friends_profile,
            friends_code,
            friends_add,
            friends_ask_near,
            friends_answer,
            friends_invite,
            friends_join,
            classic_status, // Leyline Solo (host/classic.rs, host/classicsign.rs; docs/LEYLINE-SOLO.md)
            classic_setup,
            classic_start,
            classic_progress,
            classic_stop,
            classic_logs,
            classic_update_check,
            classic_settings,
            classic_open_logs,
            classic_play,
            classic_password,
            classic_signin_off,
            classic_characters,
            classic_choices,
            classic_invite,
            classic_friends,
            classic_share_stop,
            bossrush_catalog, // Boss Rush (host/bossrush.rs, ui/bossrush.js)
            bossrush_setup,
            bossrush_start,
            bossrush_stop,
            bossrush_logs,
            bossrush_play,
            bossrush_board,
            bossrush_record,
            bossrush_code,
            boards_get, // Leaderboards: signed boards checked here, local runs shared by choice (boards.rs, ui/boards.js)
            boards_share_local,
            bossrush_start_at, // start anywhere (ui/bossstart.js)
            bossrush_state,
            bossrush_retry,
            bossrush_go_now,
            bossrush_leave,
            bossrush_invite,
            forge_state, // Forge, the Den's terrain editor (host/forge.rs, ui/den.js "Open in Forge")
            forge_install,
            forge_open,
            crate::p2p::cmds::host_den_open,
            crate::p2p::cmds::host_modes, // modes anyone can host and fork: cards, fork, change, remove, host one (host/modes.rs)
            crate::p2p::cmds::host_mode_fork,
            crate::p2p::cmds::host_mode_save,
            crate::p2p::cmds::host_mode_remove,
            crate::p2p::cmds::host_mode_use,
            crate::p2p::cmds::host_flow_apply, // the Host page's flow: templates, the settings panel, Ask Claude (host/flow.rs)
            crate::p2p::cmds::host_flow_current,
            crate::p2p::cmds::host_plan_ask,
            crate::p2p::cmds::host_plan_take,
            crate::p2p::cmds::host_check,
            crate::p2p::cmds::host_tool,
            crate::p2p::cmds::host_rules,
            crate::p2p::cmds::host_rules_set,
            crate::p2p::cmds::host_link_start,
            crate::p2p::cmds::host_start_quiet, // no window: the Host page's answers, its steps (host.rs plan_quiet, progress)
            crate::p2p::cmds::host_link_quiet,
            crate::p2p::cmds::host_progress,
            crate::p2p::cmds::host_scan, // a server already on this PC, to link (host/scan.rs)
            crate::p2p::cmds::host_network, // the host console's Network tab (host/network.rs)
            crate::p2p::cmds::host_network_set,
            crate::p2p::cmds::host_herokey_test,
            crate::p2p::cmds::host_carry, // Help carry the network (host/network.rs)
            crate::p2p::cmds::host_carry_set,
            crate::p2p::cmds::node_state, // Be a node: the Host page's one switch (p2p/node.rs)
            crate::p2p::cmds::node_set,
            crate::p2p::cmds::node_check_network,
            crate::p2p::cmds::node_quit,
            crate::p2p::cmds::p2p_flags,
            crate::p2p::cmds::p2p_set_flag,
            crate::p2p::cmds::p2p_set_limits,
            crate::p2p::cmds::p2p_nat_check,
            // shared magnet: links, behind the magnet_links flag (src/p2p/magnet.rs)
            crate::p2p::cmds::magnet_open,
            crate::p2p::cmds::magnet_pending,
            crate::p2p::cmds::magnet_dismiss,
            crate::p2p::cmds::magnet_fetch,
            // Leyline friends and messages (social.rs)
            crate::social::commands::social_status,
            crate::social::commands::social_signin_start,
            crate::social::commands::social_signin_reopen,
            crate::social::commands::social_signin_poll,
            crate::social::commands::social_signin_cancel,
            crate::social::commands::social_call,
            crate::social::commands::social_signout,
            crate::social::commands::social_ways,
            // Your Hero Key in the launcher (Manage account; stream/herokeys.py)
            crate::social::commands::social_key_signin,
            crate::social::commands::social_key_file,
            crate::social::commands::social_key_found,
            crate::social::commands::social_new_account,
            crate::social::commands::social_password_signin,
            crate::social::commands::save_text_file,
            // the Forum page: reading the website's forum, opening its pages (forum.rs)
            crate::forum::commands::forum_call,
            crate::bugs::commands::bug_report,
            crate::forum::commands::forum_post,
            crate::forum::commands::forum_open,
            // List your mod: check, list, report, edit, remove with the launcher's sign-in (listing.rs)
            // Mods & Addons › Dens: the shared Dens and stamps (denmarket.rs, stream/denmarket.py)
            crate::denmarket::commands::dens_browse,
            crate::denmarket::commands::dens_detail,
            crate::denmarket::commands::dens_mine,
            crate::denmarket::commands::dens_post,
            crate::denmarket::commands::dens_share,
            crate::denmarket::commands::dens_open,
            crate::denmarket::commands::dens_stamp,
            crate::denmarket::commands::dens_showcase,
            crate::denmarket::commands::dens_shots,
            crate::denmarket::commands::dens_page_open,
            crate::listing::commands::listing_post,
            crate::listing::commands::listing_mine,
            crate::listing::commands::listing_open,
            // the News page: the website's news feed (news.rs)
            crate::news::commands::news_list,
            // the Modes page (modes.rs)
            crate::modes::commands::modes_list,
            crate::modes::commands::mode_open_browser,
            crate::modes::commands::mode_open_credit,
            crate::modes::commands::mode_prepare_desktop,
            // the Leyline addons: our own in-game addons in a 3.3.5a client (addons.rs, ui/addons.js)
            crate::addons::commands::addons_status,
            crate::addons::commands::addons_install,
            crate::addons::commands::addons_remove,
            crate::addons::commands::addons_auto_update,
            crate::addons::commands::addons_set_prefs,
            crate::addons::commands::addons_should_offer,
            crate::addons::commands::addons_before_play,
            crate::addons::commands::addons_play_note,
            crate::addons::commands::addons_mark_asked,
            // Agents: Allow agents, approval cards, the audit, Connect (agents.rs, ui/agents.js)
            crate::agents::commands::agents_state,
            crate::agents::commands::agents_set_allow,
            crate::agents::commands::agents_answer,
            crate::agents::commands::agents_connect
        ])
        .run(tauri::generate_context!())
        .expect("error while running Leyline");
}

#[cfg(test)]
mod solo_tests {
    use super::*;

    /// PLAY on the official realm (owner 2 Nov): what the player reads after "Started …", in each case, in plain
    /// words with no version number and never a password.
    #[test]
    fn play_on_the_official_realm_says_how_it_gets_past_the_login_screen() {
        let ok: Result<(), String> = Ok(());
        let n = official_note(&ok, &Ok(()));
        assert!(n.contains("signs you in by itself") && n.contains("character select"), "{n}");
        let n = official_note(&ok, &Err(String::new()));
        assert!(n.contains("account name is filled in") && !n.contains("didn't work"), "the switch off: {n}");
        let n = official_note(&ok, &Err("Forever UI's login screens aren't in this game folder yet".into()));
        assert!(n.contains("account name is filled in") && n.contains("didn't work here: Forever UI"), "{n}");
        let n = official_note(&Err("signed_out".into()), &Err(String::new()));
        assert!(n.contains("Log in to Leyline"), "{n}");
        let n = official_note(&Err("The website isn't answering".into()), &Err(String::new()));
        assert!(n.contains("didn't work (The website") && n.contains("game password"), "{n}");
        for n in [official_note(&ok, &Ok(())), official_note(&ok, &Err(String::new())), official_note(&Err("signed_out".into()), &Ok(()))] {
            assert!(!n.chars().any(|c| c.is_ascii_digit()) && !n.to_lowercase().contains("warcraft"), "{n}");
        }
    }

    /// The official realm's login goes into the hybrid archive for one start, as Your Den's does, with `enter: false`
    /// (it stops at character select), and comes out again with the scrub; Config.wtf gets the account name only.
    #[test]
    fn the_official_login_goes_in_for_one_start_and_out_again() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Data/enUS")).unwrap();
        std::fs::create_dir_all(d.path().join("WTF")).unwrap();
        let g = d.path().join("Interface/GlueXML");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("GlueXML.toc"), "GlueParent.xml\r\nForeverUIGlue.xml\r\n").unwrap();
        std::fs::write(g.join("ForeverUIGlue.xml"), "<Ui/>").unwrap();
        let l = crate::herokey::parse_game_login(&serde_json::json!({"ok": true, "user": "WR000123", "pass": "Q7W8E9R2T3Y4U5I6"})).unwrap();
        assert!(crate::herokey::set_account_name(d.path(), &l.user).unwrap());
        let s = crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: false, pick: None, make: None };
        assert!(crate::hybrid::session_lua(Some(&s)).unwrap().contains("enter = false"), "character select, not the world");
        crate::hybrid::write(d.path(), Some(&s)).unwrap();
        assert!(crate::hybrid::has_session(d.path()));
        let cfg = std::fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert!(cfg.contains("SET accountName \"WR000123\"") && !cfg.contains("Q7W8"), "{cfg}");
        assert!(crate::hybrid::scrub(d.path()).unwrap() && !crate::hybrid::has_session(d.path()), "gone after the start");
    }

    /// My Den's sign-in: Den Lite's fixed local login goes into the hybrid archive (escaped, for one start) and
    /// the account and realm into Config.wtf, never the password; without Forever UI's login screens nothing is
    /// written to Data (the launcher's keys sign in instead: denclick.rs).
    #[test]
    fn play_solo_signs_in_to_the_local_den_by_itself() {
        use crate::host::lite;
        let s = den_session();
        assert_eq!((s.user.as_str(), s.pass.as_str(), s.realm.as_deref(), s.enter), (crate::denclick::GLUE_ACCOUNT, lite::PASSWORD, Some(lite::REALM), true),
            "straight into the world, under the login screen code's own account name");
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Data/enUS")).unwrap();
        std::fs::create_dir_all(d.path().join("WTF")).unwrap();
        // no Forever UI: no archive, the password is shown to type
        let r = crate::hybrid::write(d.path(), Some(&s)).map(|_| ());
        assert!(!d.path().join("Data").join(crate::hybrid::SLOT).exists());
        let out = solo_result("Started.".into(), r);
        assert!(!out.auto && out.password == lite::PASSWORD && out.why.contains("Forever UI"), "{out:?}");
        // Forever UI's glue there: the game signs in by itself
        let g = d.path().join("Interface/GlueXML");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(g.join("GlueXML.toc"), "GlueParent.xml\r\nForeverUIGlue.xml\r\n").unwrap();
        std::fs::write(g.join("ForeverUIGlue.xml"), "<Ui/>").unwrap();
        let r = crate::hybrid::write(d.path(), Some(&s)).map(|_| ());
        assert!(crate::hybrid::has_session(d.path()));
        let out = solo_result("Started.".into(), r);
        assert!(out.auto && out.password.is_empty() && out.why.is_empty(), "{out:?}");
        assert!(crate::herokey::set_login(d.path(), lite::ACCOUNT, Some(lite::REALM)).unwrap());
        let cfg = std::fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert!(cfg.contains("SET accountName \"DEN\"") && cfg.contains("SET realmName \"Your Den\"") && !cfg.contains("\"den\""), "{cfg}");
        // the game closed: the session is taken out again
        assert!(crate::hybrid::scrub(d.path()).unwrap() && !crate::hybrid::has_session(d.path()));
    }
}

// ---- single player's playtest (crate::sptest; tools/first-run/sp_playtest.py, Control: sp-playtest): the one-click
// flow through the same functions the Single player page uses, then the checks, with nobody at the keyboard.

/// The playtest's lines: stdout (the test tool keeps it) and `<data>/sp-play-test.log`.
fn sp_say(dir: &Path, line: &str) {
    use std::io::Write;
    let stamp = crate::host::engine::stamp(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
    let text = format!("{stamp} {line}\n");
    let _ = std::io::stdout().write_all(text.as_bytes());
    let _ = std::io::stdout().flush();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(crate::sptest::LOG)) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Follow a setup or repair job on engine_progress to its end (`max_s` at most), its steps said as they come.
/// `started`: the job was running right after it began (a world with nothing to do starts no job).
fn sp_follow_job(dir: &Path, what: &str, max_s: u64) -> Result<(), String> {
    use crate::host::engine as eng;
    let until = std::time::Instant::now() + Duration::from_secs(max_s);
    let mut last = (String::new(), 0usize, 255u8);
    let mut said_at = std::time::Instant::now();
    loop {
        let j = eng::progress();
        let pct = j.pct.unwrap_or(255);
        let moved = j.step != last.0 || j.n != last.1 || (pct != 255 && (last.2 == 255 || pct >= last.2.saturating_add(10)));
        if moved || said_at.elapsed() > Duration::from_secs(120) {
            let p = j.pct.map(|p| format!(" {p}%")).unwrap_or_default();
            let detail = if j.detail.is_empty() { String::new() } else { format!(" ({})", j.detail.chars().take(120).collect::<String>()) };
            sp_say(dir, &format!("{what}: step {}/{} {}{p}{detail}", j.n, j.of, j.step));
            last = (j.step.clone(), j.n, pct);
            said_at = std::time::Instant::now();
        }
        if !j.running {
            return match j.problem {
                Some(p) => Err(p),
                None => Ok(()),
            };
        }
        if std::time::Instant::now() > until {
            return Err(format!("[other] {what} took longer than {} minutes", max_s / 60));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// The character of `user` in this world now (online first), read from its database.
fn sp_char(p: &crate::host::engine::Paths, user: &str) -> Option<crate::sptest::Char> {
    let sql = crate::sptest::in_world_sql(user).ok()?;
    crate::sptest::parse_char(&crate::host::engine::query(p, "acore_characters", &sql).ok()?)
}

/// The worldserver's logs say this character entered the world.
fn sp_log_says(p: &crate::host::engine::Paths, name: &str) -> bool {
    std::fs::read_dir(&p.logs)
        .map(|rd| {
            rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("log"))).any(|e| {
                std::fs::read(e.path()).map(|b| crate::sptest::log_in_world(&String::from_utf8_lossy(&b), name)).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Wait for `user`'s character to be in the world (characters.online = 1; the worldserver log's login line is said
/// beside it) while the game `pid` runs. Ok(character, how it was seen), Err(why).
fn sp_wait_in_world(p: &crate::host::engine::Paths, user: &str, pid: u32, max_s: u64) -> Result<(crate::sptest::Char, &'static str), String> {
    use crate::denclick::{Game, Process};
    let until = std::time::Instant::now() + Duration::from_secs(max_s);
    let mut last: Option<crate::sptest::Char> = None;
    while std::time::Instant::now() < until {
        if let Some(c) = sp_char(p, user) {
            if c.online {
                let how = if sp_log_says(p, &c.name) { "online in the world's database; its log has the login line" } else { "online in the world's database" };
                return Ok((c, how));
            }
            last = Some(c);
        }
        if pid != 0 && !Process(pid).alive() {
            return Err(format!("the game closed before the world{}", last.map(|c| format!(" ({} made, level {})", c.name, c.level)).unwrap_or_default()));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(match last {
        Some(c) if sp_log_says(p, &c.name) => format!("{} is in the world's log but not online in its database after {max_s} s", c.name),
        Some(c) => format!("{} was made but never entered the world in {max_s} s", c.name),
        None => format!("no character on this login after {max_s} s (Quick start didn't make one)"),
    })
}

/// Close every game this run started (kill, then wait).
fn sp_close_games(core: &Core) {
    if let Ok(mut g) = core.games.lock() {
        for c in g.iter_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
        g.clear();
    }
}

fn sp_ports_closed() -> bool {
    use crate::host::engine as eng;
    !eng::port_answers(eng::LOGIN_PORT) && !eng::port_answers(eng::WORLD_PORT) && !eng::port_answers(eng::DB_PORT)
}

/// The world's environment as its server gets it (the profile, modules.env, realm.env), for the companions' count.
fn sp_world_env(p: &crate::host::engine::Paths) -> Vec<(String, String)> {
    use crate::host::engine as eng;
    let read = |f: PathBuf| std::fs::read_to_string(f).unwrap_or_default();
    let realm = std::fs::read_to_string(p.realm_env()).ok();
    eng::world_env(p, &eng::load_state(p), &read(p.profile_env(p.kind.profile())), &read(p.modules_env()), realm.as_deref())
}

/// `Leyline play-test --single <single|vanilla> [--fresh] [--game <folder>] [--hold <s>] [--level <n>] [--checks …]`
/// (crate::sptest): single player in one click with nobody at the keyboard, through the page's own functions:
/// world_find_folder → world_preflight → world_setup (its progress on stdout) → world_play with `first` (a random Quick
/// start) → in the world; then the checks. Writes `<data>/sp-play-test.json` (no password, no key). Exit 0 only when
/// every step passed.
pub fn single_play_test_cli(cli: crate::playtest::Cli) -> i32 {
    use crate::host::{engine as eng, oneclick as oc};
    use crate::sptest::{self as sp, Check};
    std::hint::black_box(sp::SP_MARK.as_ptr());
    let dir = settings::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Some(k) = cli.single.as_deref().and_then(eng::Kind::from_word).filter(|k| *k != eng::Kind::BossRush) else {
        eprintln!("--single takes single or vanilla");
        return 2;
    };
    let now = crate::denclick::now_ms;
    let mut rep = sp::Report::new(k.word(), &cli.sp_checks, now());
    let report_path = dir.join(sp::REPORT);
    let save = |rep: &sp::Report| {
        let _ = settings::write_atomic(&report_path, serde_json::to_string_pretty(rep).unwrap_or_default().as_bytes());
    };
    let say = |l: &str| sp_say(&dir, l);
    let end = |mut rep: sp::Report| -> i32 {
        rep.finish(now());
        let _ = settings::write_atomic(&dir.join(sp::REPORT), serde_json::to_string_pretty(&rep).unwrap_or_default().as_bytes());
        sp_say(&dir, &format!("{}\n{}", rep.table(), if rep.passed { "PASS" } else { "FAIL" }));
        if rep.passed { 0 } else { 1 }
    };
    let want = |c: Check| cli.sp_checks.contains(&c);
    say(&format!("single player playtest: {} ({}), checks: {}", k.title(), k.word(), cli.sp_checks.iter().map(|c| c.word()).collect::<Vec<_>>().join(",")));
    // --fresh: main.rs pointed the data folder at an empty one; a launcher's settings in it means it isn't
    let had = dir.join(settings::FILE_NAME).exists();
    rep.extra["fresh"] = serde_json::json!({ "asked": cli.fresh, "empty": !had, "dir": dir });
    if cli.fresh && had {
        rep.step("fresh data folder", Some(false), 0, "--fresh needs an empty data folder: this one already has a launcher's settings");
        return end(rep);
    }
    if !eng::supported() {
        rep.step("setup", Some(false), 0, "the Built-in engine is made for Windows PCs: run this on the PC");
        return end(rep);
    }
    let core = Core {
        settings: Mutex::new(settings::load(&dir)),
        dir: dir.clone(),
        known_urls: Mutex::new(HashSet::new()),
        mods_lock: Arc::new(Mutex::new(())),
        games: Mutex::new(Vec::new()),
        upd: Mutex::new(UpdState::default()),
        deep_link: Mutex::new(crate::deeplink::Inbox::default()),
    };
    let p = world_paths(k);
    // the test Hero Key (the first-run tool's test account), only from the environment: signed in, then forgotten
    let t = std::time::Instant::now();
    match std::env::var_os(crate::playtest::KEY_ENV) {
        Some(key) => {
            std::env::remove_var(crate::playtest::KEY_ENV);
            let r = crate::social::sign_in_key(&dir, &key.to_string_lossy());
            drop(key);
            match r {
                Ok(_) => rep.step("Hero Key", Some(true), t.elapsed().as_millis() as u64, "signed in with the test Hero Key: the world plays on the Hero Key's own login"),
                Err(e) => rep.step("Hero Key", Some(false), t.elapsed().as_millis() as u64, format!("the test Hero Key didn't sign in: {e}")),
            }
        }
        None => rep.step("Hero Key", None, 0, "no test Hero Key: the world's own login (as offline)"),
    }
    save(&rep);
    // 1. the game folder: --game added as Find your game files does, then world_find_folder's pick
    let t = std::time::Instant::now();
    if let Some(g) = &cli.game {
        let same = |a: &Path| a.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(g.to_string_lossy().trim_end_matches(['\\', '/']));
        if !core.snapshot().clients.iter().any(|c| same(&c.path)) {
            if let Err(e) = clients::detect(g).map_err(|e| e.to_string()).and_then(|det| add_profile(&core, g.clone(), det)) {
                rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, format!("couldn't add the game folder: {e}"));
                return end(rep);
            }
        }
    }
    let found = find_folder_now(&core, k);
    let Some(client) = found.picked.clone() else {
        rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, format!("world_find_folder picked none ({} found)", found.found.len()));
        return end(rep);
    };
    let profile = match world_client(&core, k, &client) {
        Ok(pr) => pr,
        Err(e) => {
            rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, e);
            return end(rep);
        }
    };
    rep.step("game folder", Some(true), t.elapsed().as_millis() as u64, format!("{} ({})", profile.name, profile.path.display()));
    // 2. the disk check
    let t = std::time::Instant::now();
    let pf = eng::preflight(&p);
    rep.step("preflight", Some(pf.ok), t.elapsed().as_millis() as u64, if pf.ok { format!("{:.1} GB free, {:.1} GB needed", pf.free_gb, pf.need_gb) } else { pf.message.clone() });
    save(&rep);
    if !pf.ok {
        return end(rep);
    }
    // 3. setup (the engine, the maps from the game folder, the database, the login) and the start
    rep.phase = "setup".into();
    save(&rep);
    let t = std::time::Instant::now();
    let setup = world_setup_with(k, &profile).and_then(|line| {
        say(&format!("setup: {line}"));
        sp_follow_job(&dir, "setup", 200 * 60)
    });
    let st = eng::status(&p);
    let ok = setup.is_ok() && st.running;
    rep.step(
        "setup",
        Some(ok),
        t.elapsed().as_millis() as u64,
        match &setup {
            Err(e) => e.clone(),
            Ok(()) if !st.running => format!("set up {} but not running (database {}, login {}, world {})", st.set_up, st.db_up, st.auth_up, st.world_up),
            Ok(()) => format!("engine {}, set up and running", if st.version.is_empty() { "?".to_string() } else { st.version.clone() }),
        },
    );
    save(&rep);
    if !ok {
        return end(rep);
    }
    // 4. Play, as the card's first Play: a random Quick start, straight into the world
    rep.phase = "entering".into();
    let t = std::time::Instant::now();
    let hero = oc::hero_identity(&core.dir);
    let played = world_play_now(&core, &client, k, None, None, true, hero, None);
    let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
    let play = match played {
        Ok(pl) => pl,
        Err(e) => {
            rep.step("play", Some(false), t.elapsed().as_millis() as u64, e);
            return end(sp_stop_all(&core, &profile, k, rep));
        }
    };
    rep.pid = pid;
    rep.account = play.account.clone();
    rep.hero = play.hero;
    rep.step("play", Some(pid != 0), t.elapsed().as_millis() as u64, format!("the game started (pid {pid}), Quick start: {}", if play.made.is_empty() { "-".to_string() } else { play.made.clone() }));
    rep.step("signed in by itself", Some(play.auto), 0, if play.auto { "the login screen got the world's login" } else { "the game was left at its login screen" });
    save(&rep);
    // 5. in the world
    let t = std::time::Instant::now();
    let me = match sp_wait_in_world(&p, &play.account, pid, sp::IN_WORLD_S) {
        Ok((c, how)) => {
            rep.step("in the world", Some(true), t.elapsed().as_millis() as u64, format!("{} level {} ({how})", c.name, c.level));
            rep.character = Some(c.clone());
            c
        }
        Err(e) => {
            rep.step("in the world", Some(false), t.elapsed().as_millis() as u64, e);
            return end(sp_stop_all(&core, &profile, k, rep));
        }
    };
    let in_world_at = std::time::Instant::now();
    let hold = Duration::from_secs(cli.hold.clamp(8, crate::playtest::HOLD_MAX));
    rep.phase = "in_world".into();
    save(&rep);
    // 6. companions: in the group and online within 30 s of entering
    let expected = sp::expected_companions(&sp_world_env(&p));
    rep.extra["companions_expected"] = expected.into();
    if want(Check::Companions) {
        if expected == 0 {
            rep.step("companions", None, 0, "this world's settings have no companions joining by themselves");
        } else {
            let mut seen_at: Option<u64> = None;
            let mut rows = Vec::new();
            let mut why = String::new();
            while in_world_at.elapsed() < Duration::from_secs(sp::COMPANIONS_WATCH_S) {
                match eng::query(&p, "acore_characters", &sp::companions_sql(me.guid)) {
                    Ok(r) => rows = sp::parse_companions(&r),
                    Err(e) => why = e,
                }
                if sp::with_player(&rows).len() as u32 >= expected {
                    seen_at = Some(in_world_at.elapsed().as_secs());
                    break;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            let with = sp::with_player(&rows);
            let names = with.iter().map(|c| format!("{} ({}, level {})", c.name, c.role, c.level)).collect::<Vec<_>>().join(", ");
            let ok = seen_at.is_some_and(|s| s <= sp::COMPANIONS_WITHIN_S);
            let line = match seen_at {
                Some(s) => format!("{}/{expected} in the group and online after {s} s: {names}", with.len()),
                None if !why.is_empty() => format!("{}/{expected} after {} s; the database: {why}", with.len(), sp::COMPANIONS_WATCH_S),
                None => format!("{}/{expected} after {} s: {}", with.len(), sp::COMPANIONS_WATCH_S, if names.is_empty() { "none".to_string() } else { names }),
            };
            rep.step("companions", Some(ok), in_world_at.elapsed().as_millis() as u64, line);
            rep.companions = rows.clone();
        }
        save(&rep);
    }
    // the game stays in the world `--hold` seconds from entering (the screenshots)
    if let Some(rest) = hold.checked_sub(in_world_at.elapsed()) {
        std::thread::sleep(rest);
    }
    // 7. levelup: server-driven, on this local world only (SOAP 127.0.0.1), each level read back from the database
    if want(Check::Levelup) {
        let t = std::time::Instant::now();
        let mut level = me.level;
        let mut why = String::new();
        rep.levels.push(level);
        while level < cli.level_to && why.is_empty() {
            let next = level + 1;
            let r = sp::levelup_command(&me.name, next).and_then(|c| eng::console(&p, &c));
            if let Err(e) = r {
                why = format!("level {next}: the world's console said no: {e}");
                break;
            }
            // saved, then read back (a few tries: the save is queued)
            let mut got = 0u8;
            for _ in 0..8 {
                std::thread::sleep(Duration::from_secs(2));
                let _ = eng::console(&p, ".saveall");
                std::thread::sleep(Duration::from_secs(1));
                got = eng::query(&p, "acore_characters", &sp::level_sql(me.guid)).ok().and_then(|r| r.trim().parse().ok()).unwrap_or(0);
                if got >= next {
                    break;
                }
            }
            if got != next {
                why = format!("asked for level {next}, the saved character is level {got}");
                break;
            }
            level = got;
            rep.levels.push(level);
            say(&format!("levelup: {} is level {level} (saved)", me.name));
            save(&rep);
            std::thread::sleep(Duration::from_secs(2));
        }
        let mut line = if why.is_empty() { format!("{} {} to {level}, each level saved and read back ({})", me.name, me.level, sp::LEVEL_NOTE) } else { why.clone() };
        let mut ok = why.is_empty();
        // the companions keep to the player's level (the script re-levels them on the level change)
        if ok && expected > 0 {
            let mut with: Vec<(String, u8)> = Vec::new();
            let until = std::time::Instant::now() + Duration::from_secs(120);
            while std::time::Instant::now() < until {
                let _ = eng::console(&p, ".saveall");
                std::thread::sleep(Duration::from_secs(2));
                let rows = eng::query(&p, "acore_characters", &sp::companions_sql(me.guid)).map(|r| sp::parse_companions(&r)).unwrap_or_default();
                with = sp::with_player(&rows).iter().map(|c| (c.name.clone(), c.level)).collect();
                if sp::follow_level(&with, level) {
                    break;
                }
                std::thread::sleep(Duration::from_secs(3));
            }
            let follow = sp::follow_level(&with, level);
            line.push_str(&format!(
                "; companions {}: {}",
                if follow { "followed" } else { "did NOT follow" },
                if with.is_empty() { "none with the player".to_string() } else { with.iter().map(|(n, l)| format!("{n} {l}")).collect::<Vec<_>>().join(", ") }
            ));
            ok &= follow;
        }
        rep.step("levelup", Some(ok), t.elapsed().as_millis() as u64, line);
        if let Some(c) = sp_char(&p, &play.account) {
            rep.character = Some(c);
        }
        rep.phase = "levelled".into();
        save(&rep);
        std::thread::sleep(Duration::from_secs(8));
    }
    // 8. stop on exit: the game closed, the world stops by itself (the watcher world_play started)
    if want(Check::StopOnExit) {
        let setting = settings::load(&dir).world_stop_on_exit;
        rep.phase = "closing".into();
        save(&rep);
        sp_close_games(&core);
        let t = std::time::Instant::now();
        let mut stopped_at = None;
        while t.elapsed() < Duration::from_secs(sp::STOP_WITHIN_S) {
            std::thread::sleep(Duration::from_secs(3));
            if sp_ports_closed() {
                stopped_at = Some(t.elapsed().as_secs());
                break;
            }
        }
        let ok = setting && stopped_at.is_some();
        rep.step(
            "stoponexit",
            Some(ok),
            t.elapsed().as_millis() as u64,
            match stopped_at {
                Some(s) => format!("the world stopped {s} s after the game closed (setting on: {setting})"),
                None => format!("the world still answers {} s after the game closed (setting on: {setting})", sp::STOP_WITHIN_S),
            },
        );
        save(&rep);
    }
    // 9. repair: a non-critical engine file deleted, Repair, the file back, the world up, Play into the world again
    if want(Check::Repair) {
        sp_close_games(&core);
        let t = std::time::Instant::now();
        let manifest = oc::read_manifest(&p.dist);
        let all = sp::files_under(&p.dist);
        let victim = match &manifest {
            Some(m) => sp::pick_victim(m.keys().map(String::as_str)),
            None => sp::pick_victim(all.iter().map(String::as_str)),
        };
        let r: Result<String, String> = (|| {
            let v = victim.ok_or("no file in the engine the world can do without (no .txt, .sql, .dist or .lua)")?;
            let f = sp::rel_path(&p.dist, &v);
            let before = std::fs::metadata(&f).map(|m| m.len()).map_err(|e| format!("{v}: {e}"))?;
            std::fs::remove_file(&f).map_err(|e| format!("couldn't delete {v}: {e}"))?;
            say(&format!("repair: deleted {v} ({before} bytes)"));
            let line = world_repair_now(&core, k, &client)?;
            say(&format!("repair: {line}"));
            sp_follow_job(&dir, "repair", 90 * 60)?;
            let back = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
            if back != before {
                return Err(format!("{v} isn't back after Repair ({back} bytes, was {before})"));
            }
            if let Some(m) = &manifest {
                if !oc::manifest_bad(&p.dist, m).is_empty() {
                    return Err(format!("Repair finished but engine files still differ from engine.json: {}", oc::manifest_bad(&p.dist, m).join(", ")));
                }
            }
            if !eng::status(&p).running {
                return Err("Repair finished but the world isn't running".into());
            }
            // Play again (the last character, no Quick start) and into the world
            let hero = oc::hero_identity(&core.dir);
            let again = world_play_now(&core, &client, k, None, None, false, hero, None)?;
            let pid2 = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
            rep.pid = pid2;
            rep.phase = "entering_again".into();
            save(&rep);
            let (c, _) = sp_wait_in_world(&p, &again.account, pid2, sp::IN_WORLD_S)?;
            Ok(format!("{v} deleted and put back by Repair ({}), the world up, Play again: {} in the world", if manifest.is_some() { "checked against engine.json" } else { "engine without a file list" }, c.name))
        })();
        let ok = r.is_ok();
        rep.step("repair", Some(ok), t.elapsed().as_millis() as u64, r.unwrap_or_else(|e| e));
        if ok {
            rep.phase = "repaired_in_world".into();
            save(&rep);
            std::thread::sleep(Duration::from_secs(8));
        }
        save(&rep);
    }
    end(sp_stop_all(&core, &profile, k, rep))
}

/// Boss Rush's status for the playtest: the run as the module sees it, or why not (offline, an error).
fn br_status() -> Result<crate::host::bossrush::RushRun, String> {
    use crate::host::bossrush::Rush;
    match bossrush_rush_now("status", None)? {
        Rush::Run(r) => Ok(r),
        Rush::Lobby { waiting, size, .. } => Err(format!("waiting for players ({waiting} of {size})")),
        Rush::Offline => Err("the hero isn't in the world yet".into()),
        Rush::No { why } => Err(why),
        Rush::Ok { word } => Err(format!("unexpected answer: {word}")),
        Rush::Nothing => Err("the realm gave no answer (an engine older than start anywhere?)".into()),
    }
}

/// Wait (at most `max_s`) until `ok` says the run is where it should be; the last status and its verdict either way.
fn br_wait(max_s: u64, mut ok: impl FnMut(&crate::host::bossrush::RushRun) -> Result<String, String>) -> (Option<crate::host::bossrush::RushRun>, Result<String, String>) {
    let until = std::time::Instant::now() + Duration::from_secs(max_s);
    let mut last = (None, Err("never looked".to_string()));
    while std::time::Instant::now() < until {
        match br_status() {
            Ok(r) => {
                let v = ok(&r);
                let done = v.is_ok();
                last = (Some(r), v);
                if done {
                    return last;
                }
            }
            Err(e) => last.1 = Err(e),
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    last
}

/// `Leyline play-test --bossrush [--fresh] [--game <folder>] [--boss <start>] [--size 10|25] [--diff normal|heroic]
/// [--hero <class>-<spec>] [--hold <s>] [--fight-max <s>] [--checks bossstart,bossfight]` (crate::sptest; owner 12 Nov:
/// "tested by you at least a full boss fight run through"): Boss Rush through the page's own functions with nobody
/// at the keyboard: the Boss Rush install set up from the game folder (bossrush_setup's begin_kind), then the start
/// picker's one click (bossrush_start_at's bossrush_exact: the hero, its waiting go, the game signed in), then
/// * `bossstart`: the raid in front of the boss, checked on the realm itself (`bossfight rush status`: the start's
///   map, an instance of its own, the hero within a few yards of the start, a full raid of companions inside);
/// * `bossfight`: the hero handed to the bots' own AI (`bossfight rush autopilot on`: nobody is at the keys), the
///   fight begun as a player would (`bossfight rush pull`: the final fight's paladin spoken to), then watched to its
///   end: the engage, the boss's phase casts, the outcome (a kill or a wipe, as it was), then Retry (a wipe: the
///   heroes' own auto retry, else `bossfight rush retry`; a clear: the same start in a fresh instance) until the raid
///   is in front of the boss again. Bots that can't win don't fail it: a fight that never starts or ends, or can't be
///   retried, does.
/// Writes `<data>/br-play-test.json`. Exit 0 only when every step passed.
pub fn bossrush_play_test_cli(cli: crate::playtest::Cli) -> i32 {
    use crate::host::{bossrush as br, engine as eng};
    use crate::sptest::{self as sp, Check};
    std::hint::black_box(sp::BR_MARK.as_ptr());
    let dir = settings::app_dir();
    let _ = std::fs::create_dir_all(&dir);
    let k = eng::Kind::BossRush;
    let now = crate::denclick::now_ms;
    let mut rep = sp::Report::new("bossrush", &cli.sp_checks, now());
    rep.levelling = "";
    let report_path = dir.join(sp::BR_REPORT);
    let save = |rep: &sp::Report| {
        let _ = settings::write_atomic(&report_path, serde_json::to_string_pretty(rep).unwrap_or_default().as_bytes());
    };
    let say = |l: &str| sp_say(&dir, l);
    let end = |mut rep: sp::Report| -> i32 {
        rep.finish(now());
        let _ = settings::write_atomic(&dir.join(sp::BR_REPORT), serde_json::to_string_pretty(&rep).unwrap_or_default().as_bytes());
        sp_say(&dir, &format!("{}\n{}", rep.table(), if rep.passed { "PASS" } else { "FAIL" }));
        if rep.passed { 0 } else { 1 }
    };
    let want = |c: Check| cli.sp_checks.contains(&c);
    let (class, spec) = cli.hero.split_once('-').unwrap_or(("mage", "fire"));
    let diff = if cli.br_heroic { "heroic" } else { "normal" };
    say(&format!("Boss Rush playtest: {} ({diff}, {} players), hero {class} {spec}, checks: {}", cli.boss, cli.br_size, cli.sp_checks.iter().map(|c| c.word()).collect::<Vec<_>>().join(",")));
    let had = dir.join(settings::FILE_NAME).exists();
    rep.extra["fresh"] = serde_json::json!({ "asked": cli.fresh, "empty": !had, "dir": dir });
    rep.extra["start"] = serde_json::json!({ "start": cli.boss, "diff": diff, "size": cli.br_size, "hero": cli.hero });
    if cli.fresh && had {
        rep.step("fresh data folder", Some(false), 0, "--fresh needs an empty data folder: this one already has a launcher's settings");
        return end(rep);
    }
    if !eng::supported() {
        rep.step("setup", Some(false), 0, "Boss Rush's exact engine is made for Windows PCs: run this on the PC");
        return end(rep);
    }
    let Some((scope, title, raid)) = br::start(&cli.boss) else {
        rep.step("setup", Some(false), 0, format!("{} isn't a Boss Rush start", cli.boss));
        return end(rep);
    };
    let pick = match br::check_start(&cli.boss, diff, cli.br_size, "companions", true, class, spec) {
        Ok(p) => p,
        Err(e) => {
            rep.step("setup", Some(false), 0, e);
            return end(rep);
        }
    };
    let core = Core {
        settings: Mutex::new(settings::load(&dir)),
        dir: dir.clone(),
        known_urls: Mutex::new(HashSet::new()),
        mods_lock: Arc::new(Mutex::new(())),
        games: Mutex::new(Vec::new()),
        upd: Mutex::new(UpdState::default()),
        deep_link: Mutex::new(crate::deeplink::Inbox::default()),
    };
    let p = bossrush_paths();
    // 1. the game folder
    let t = std::time::Instant::now();
    if let Some(g) = &cli.game {
        let same = |a: &Path| a.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(g.to_string_lossy().trim_end_matches(['\\', '/']));
        if !core.snapshot().clients.iter().any(|c| same(&c.path)) {
            if let Err(e) = clients::detect(g).map_err(|e| e.to_string()).and_then(|det| add_profile(&core, g.clone(), det)) {
                rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, format!("couldn't add the game folder: {e}"));
                return end(rep);
            }
        }
    }
    let found = find_folder_now(&core, k);
    let Some(client) = found.picked.clone() else {
        rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, format!("no single game folder of the right version ({} found)", found.found.len()));
        return end(rep);
    };
    let profile = match world_client(&core, k, &client) {
        Ok(pr) => pr,
        Err(e) => {
            rep.step("game folder", Some(false), t.elapsed().as_millis() as u64, e);
            return end(rep);
        }
    };
    rep.step("game folder", Some(true), t.elapsed().as_millis() as u64, format!("{} ({})", profile.name, profile.path.display()));
    // 2. the disk check, 3. setup (the engine, the raid maps from the game folder, the database) and the start
    let t = std::time::Instant::now();
    let pf = eng::preflight(&p);
    rep.step("preflight", Some(pf.ok), t.elapsed().as_millis() as u64, if pf.ok { format!("{:.1} GB free, {:.1} GB needed", pf.free_gb, pf.need_gb) } else { pf.message.clone() });
    if !pf.ok {
        return end(rep);
    }
    rep.phase = "setup".into();
    save(&rep);
    let t = std::time::Instant::now();
    let mut setup = eng::begin_kind(&settings::app_dir(), k, Some(profile.path.clone()), false).and_then(|line| {
        say(&format!("setup: {line}"));
        sp_follow_job(&dir, "setup", 200 * 60)
    });
    if setup.is_ok() && !eng::status(&p).running {
        setup = eng::begin_kind(&settings::app_dir(), k, None, false).and_then(|line| {
            say(&format!("start: {line}"));
            sp_follow_job(&dir, "start", 20 * 60)
        });
    }
    let st = eng::status(&p);
    let ok = setup.is_ok() && st.running;
    rep.step("setup", Some(ok), t.elapsed().as_millis() as u64, match &setup {
        Err(e) => e.clone(),
        Ok(()) if !st.running => format!("set up {} but not running (database {}, login {}, world {})", st.set_up, st.db_up, st.auth_up, st.world_up),
        Ok(()) => format!("Boss Rush's engine {}, set up and running", if st.version.is_empty() { "?".to_string() } else { st.version.clone() }),
    });
    save(&rep);
    if !ok {
        return end(rep);
    }
    // 4. the start picker's one click (the realm may still be loading: a few tries)
    rep.phase = "entering".into();
    save(&rep);
    let t = std::time::Instant::now();
    let mut played: Result<BossRushPlay, String> = Err("not tried".into());
    for _ in 0..16 {
        let pk = pick.clone();
        played = tauri::async_runtime::block_on(bossrush_exact(&core, &client, profile.clone(), move |main| br::start_go_line(main, &pk), String::new()));
        match &played {
            Err(e) if e.contains("didn't answer") || e.contains("still starting") || e.contains("gave no answer") || e.contains("didn't take") => {
                say(&format!("play: {e} (trying again in 15 s)"));
                std::thread::sleep(Duration::from_secs(15));
            }
            _ => break,
        }
    }
    let pid = LAST_GAME_PID.load(std::sync::atomic::Ordering::Relaxed);
    let play = match played {
        Ok(pl) => pl,
        Err(e) => {
            rep.step("play", Some(false), t.elapsed().as_millis() as u64, e);
            return end(sp_stop_all(&core, &profile, k, rep));
        }
    };
    rep.pid = pid;
    rep.hero = true;
    rep.account = br::hero_account(&eng::load_state(&p).account);
    rep.step("play", Some(pid != 0), t.elapsed().as_millis() as u64, format!("{} {title} ({}): the game started (pid {pid}), hero {} ({})", scope.word(), raid.title, play.hero, if play.made { "made now" } else { "ready" }));
    rep.step("signed in by itself", Some(play.auto), 0, if play.auto { "the login screen got the hero's login" } else { "the game was left at its login screen" });
    save(&rep);
    // 5. the hero in the world (the realm's own word: its status answers once the hero is in)
    let t = std::time::Instant::now();
    let (seen, v) = br_wait(sp::IN_WORLD_S, |_| Ok(String::new()));
    match (seen.as_ref(), v) {
        (Some(r), Ok(_)) => {
            rep.step("in the world", Some(true), t.elapsed().as_millis() as u64, format!("{} on map {} ({})", r.leader, r.map, if r.phase.is_empty() { "-" } else { r.phase.as_str() }));
        }
        (_, v) => {
            let e = v.err().unwrap_or_else(|| "no answer".to_string());
            rep.step("in the world", Some(false), t.elapsed().as_millis() as u64, e);
            return end(sp_stop_all(&core, &profile, k, rep));
        }
    }
    rep.phase = "in_world".into();
    save(&rep);
    // 6. bossstart: in front of the boss, with the full raid
    let start_map = raid.map;
    let size = u32::from(cli.br_size);
    let t = std::time::Instant::now();
    let (r0, v) = br_wait(sp::START_WITHIN_S, |r| sp::judge_start(r, &cli.boss, start_map, size));
    let at_start = v.is_ok();
    if want(Check::BossStart) {
        rep.step("bossstart", Some(at_start), t.elapsed().as_millis() as u64, v.clone().unwrap_or_else(|e| e));
    } else {
        rep.step("at the start", Some(at_start), t.elapsed().as_millis() as u64, v.clone().unwrap_or_else(|e| e));
    }
    rep.extra["at_start"] = serde_json::json!(r0.as_ref().map(|r| serde_json::json!({ "map": r.map, "instance": r.instance, "x": r.x, "y": r.y, "z": r.z, "dist": r.dist, "members": r.members, "bots": r.bots })));
    rep.phase = "boss_ready".into();
    save(&rep);
    let hold = Duration::from_secs(cli.hold.clamp(8, crate::playtest::HOLD_MAX));
    std::thread::sleep(hold);
    // 7. bossfight: a full fight to its end, then Retry
    if want(Check::BossFight) && at_start {
        let t = std::time::Instant::now();
        let mut timeline: Vec<serde_json::Value> = Vec::new();
        let note = |line: String, timeline: &mut Vec<serde_json::Value>| {
            say(&format!("fight: {line}"));
            timeline.push(serde_json::json!({ "s": t.elapsed().as_secs(), "what": line }));
        };
        let auto = bossrush_rush_now("autopilot", Some("on")).map(|r| rush_json(&r)).unwrap_or_else(|e| serde_json::json!({ "kind": "no", "why": e }));
        note(format!("the hero on the bots' own AI: {auto}"), &mut timeline);
        let pulled = bossrush_rush_now("pull", None).map(|r| rush_json(&r)).unwrap_or_else(|e| serde_json::json!({ "kind": "no", "why": e }));
        note(format!("pull: {pulled}"), &mut timeline);
        let before = r0.clone();
        let mut prev = before.clone();
        let mut engaged = false;
        let mut outcome: Option<(String, u64)> = None;
        let mut phases: Vec<String> = Vec::new();
        let mut shot_fighting = false;
        let until_engage = std::time::Instant::now() + Duration::from_secs(sp::ENGAGE_WITHIN_S);
        let until_end = std::time::Instant::now() + Duration::from_secs(cli.fight_max);
        while std::time::Instant::now() < until_end {
            if !engaged && std::time::Instant::now() > until_engage {
                break;
            }
            std::thread::sleep(Duration::from_secs(3));
            let Ok(cur) = br_status() else { continue };
            for e in sp::fight_events(prev.as_ref(), &cur) {
                note(e, &mut timeline);
            }
            if cur.phase == "fighting" {
                engaged = true;
                if !shot_fighting {
                    shot_fighting = true;
                    rep.phase = "fighting".into();
                    save(&rep);
                }
            }
            for m in &cur.marks {
                if !phases.contains(m) {
                    phases.push(m.clone());
                }
            }
            let base = before.as_ref().map(|b| (b.wipes, b.clears)).unwrap_or((0, 0));
            if cur.clears > base.1 || (cur.phase == "cleared") {
                engaged = true;
                outcome = Some(("clear".into(), cur.ms));
            } else if cur.wipes > base.0 || cur.phase == "wiped" {
                engaged = engaged || cur.wipes > base.0;
                outcome = Some(("wipe".into(), cur.ms));
            }
            prev = Some(cur);
            if outcome.is_some() {
                break;
            }
        }
        rep.phase = "fight_end".into();
        save(&rep);
        std::thread::sleep(Duration::from_secs(6));
        // Retry: back in front of the boss (a wipe: the heroes' own retry in 8 s, else the page's Retry; a clear: the
        // same start in a fresh instance)
        let retry: Result<String, String> = match outcome.as_ref().map(|o| o.0.as_str()) {
            None => Err("no end to retry from".into()),
            Some(what) => {
                let old_instance = prev.as_ref().map(|r| r.instance).unwrap_or(0);
                let (_, mut v) = br_wait(30, |r| if r.phase == "ready" { Ok(format!("ready again by itself (instance {})", r.instance)) } else { Err(r.phase.clone()) });
                if v.is_err() {
                    let word = bossrush_rush_now("retry", None).map(|r| rush_json(&r)).unwrap_or_else(|e| serde_json::json!({ "kind": "no", "why": e }));
                    note(format!("Retry pressed: {word}"), &mut timeline);
                    v = br_wait(sp::RETRY_WITHIN_S, |r| {
                        if r.phase != "ready" {
                            return Err(r.phase.clone());
                        }
                        if what == "clear" && r.instance == old_instance {
                            return Err("still the cleared instance".into());
                        }
                        Ok(format!("Retry: ready again (instance {})", r.instance))
                    })
                    .1;
                }
                v.and_then(|line| {
                    let (_, near) = br_wait(60, |r| if r.dist <= sp::NEAR_YD { Ok(format!("{:.1} yd from the start", r.dist)) } else { Err(format!("{:.1} yd from the start", r.dist)) });
                    near.map(|n| format!("{line}, {n}"))
                })
            }
        };
        if retry.is_ok() {
            rep.phase = "retried".into();
            save(&rep);
        }
        let _ = bossrush_rush_now("autopilot", Some("off"));
        let (ok, line) = sp::judge_fight(engaged, outcome.as_ref().map(|(w, ms)| (w.as_str(), *ms)), &phases, retry);
        rep.extra["fight"] = serde_json::json!({ "engaged": engaged, "outcome": outcome.as_ref().map(|o| o.0.clone()), "ms": outcome.as_ref().map(|o| o.1), "phases": phases, "timeline": timeline });
        rep.step("bossfight", Some(ok), t.elapsed().as_millis() as u64, line);
        save(&rep);
        std::thread::sleep(Duration::from_secs(8));
    } else if want(Check::BossFight) {
        rep.step("bossfight", Some(false), 0, "the raid never stood in front of the boss: the fight wasn't started");
    }
    end(sp_stop_all(&core, &profile, k, rep))
}

/// The end of a playtest, always: the game closed, the one-time login out of its archive, the world saved and
/// stopped, the Hero Key's sign-in forgotten.
fn sp_stop_all(core: &Core, profile: &ClientProfile, k: crate::host::engine::Kind, mut rep: crate::sptest::Report) -> crate::sptest::Report {
    use crate::host::engine as eng;
    rep.phase = "stopping".into();
    let t = std::time::Instant::now();
    sp_close_games(core);
    let mut scrubbed = false;
    for _ in 0..40 {
        if crate::hybrid::scrub(&profile.path).is_ok() {
            scrubbed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let p = world_paths(k);
    world_share_off(k);
    let stop = if eng::status(&p).running || !sp_ports_closed() { eng::stop(&p).map(|_| ()) } else { Ok(()) };
    let mut closed = sp_ports_closed();
    for _ in 0..30 {
        if closed {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
        closed = sp_ports_closed();
    }
    crate::social::sign_out(&core.dir);
    rep.step(
        "clean up",
        Some(closed && scrubbed),
        t.elapsed().as_millis() as u64,
        format!("game closed, one-time login out of the game folder: {scrubbed}, world stopped: {closed}{}", stop.err().map(|e| format!(" ({e})")).unwrap_or_default()),
    );
    rep
}
