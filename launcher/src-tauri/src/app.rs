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
use serde::Serialize;
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
    /// A `leyline://install/<id>` link Leyline was started with, until the UI takes it (deeplink.rs).
    deep_link: Mutex<Option<crate::deeplink::Link>>,
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

/// Point the client at the realm, then start it. On the official realm, signed in on Home, the game signs in by
/// itself and opens at character select (`play_official`).
#[tauri::command]
async fn play(app: AppHandle, core: State<'_, Core>, client: String, realm: String) -> Result<String, String> {
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
        crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: false }
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
    let s = crate::hybrid::Session { user: login.user.clone(), pass: login.pass.clone(), realm: None, enter: false };
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
    // a friend's Den (Den Lite): the same network first, then the friend's own address, then the relay (denways.rs)
    if let Some((user, pass)) = crate::host::litenet::guest_in(&link) {
        let profile = core.client(&client)?;
        if profile.build != clients::Build::V335a {
            return Err("Your friend's Den plays with a Wrath game folder: pick that one.".into());
        }
        let (lan, at) = crate::host::denways::ways_in(&link);
        let (way, list, tried) = blocking(move || Ok(crate::host::denways::pick(lan.as_deref(), at.as_deref(), crate::host::denways::connects))).await?;
        let (address, name) = if way == "relay" {
            let tried_now = tried.clone();
            blocking(move || realms::knock(&format!("https://{relay}"), &rid, &secret)).await.map_err(|e| {
                let before = if tried_now.is_empty() { String::new() } else { format!("{} ", tried_now.join(" ")) };
                format!("{before}The Leyline relay: {e}")
            })?
        } else {
            (list, "Your friend's Den".to_string())
        };
        let _ = crate::herokey::set_login(&profile.path, &user, None);
        let s = crate::hybrid::Session { user: user.clone(), pass: pass.clone(), realm: None, enter: false };
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
        let l = lite::Binary::find(&app);
        if !l.available() {
            return Err("Press Play solo once first: it gets your Den's server.".to_string());
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
    crate::hybrid::Session { user: crate::denclick::GLUE_ACCOUNT.into(), pass: lite::PASSWORD.into(), realm: Some(lite::REALM.into()), enter: true }
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
    }
    let l = l.with_client(game);
    let was = l.status().running;
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
    let refused = crate::hybrid::refused_marker(&profile.path).exists();
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
        let m = crate::hybrid::refused_marker(&st.game);
        let _ = std::fs::create_dir_all(m.parent().unwrap_or(&st.game));
        let _ = std::fs::write(&m, "This game closed on Leyline's login screen code; My Den signs in with the launcher's keys.\n");
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
        deep_link: Mutex::new(None),
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
        deep_link: Mutex::new(None),
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
        crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: cli.world }
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

#[tauri::command]
fn engine_progress() -> crate::host::engine::Job {
    crate::host::engine::progress()
}

/// Everyone saved, everything stopped.
#[tauri::command]
async fn engine_stop() -> Result<String, String> {
    blocking(|| crate::host::engine::stop(&engine_paths())).await
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
fn engine_play(core: State<Core>, client: String) -> Result<EnginePlay, String> {
    use crate::host::engine;
    let profile = core.client(&client)?;
    if profile.build != clients::Build::V335a {
        return Err("Single player plays with a different game folder: pick one of its version.".into());
    }
    let st = engine::status(&engine_paths());
    if !st.set_up {
        return Err("Set Single player up first: press Start on its page.".into());
    }
    if !st.running {
        return Err("Single player isn't running: press Start first.".into());
    }
    let _ = crate::herokey::set_login(&profile.path, &st.account, Some(engine::REALM_NAME));
    let (message, _) = point_and_play_with(&core, &client, engine::REALM, true, None, false)?;
    Ok(EnginePlay { message, account: st.account, password: st.password })
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

/// The `leyline://` link Leyline was started with, once (then None).
#[tauri::command]
fn deep_link_take(core: State<Core>) -> Option<crate::deeplink::Link> {
    core.deep_link.lock().ok().and_then(|mut d| d.take())
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
            deep_link: Mutex::new(crate::deeplink::from_args(std::env::args())),
        })
        .setup(|app| {
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
        let s = crate::hybrid::Session { user: l.user.clone(), pass: l.pass.clone(), realm: None, enter: false };
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
