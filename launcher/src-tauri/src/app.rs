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
use std::path::PathBuf;
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
    point_and_play_with(core, client, realm, start, None).map(|(m, _)| m)
}

/// `point_and_play`, plus the hybrid client (docs/HYBRID-CLIENT.md): with Settings' hybrid_login on and a 3.3.5a
/// client, `session` is handed to the login screen through Leyline's patch archive; without one, a session left
/// from an earlier start is taken out first, so a plain start never signs in by itself. Returns whether the
/// session went in (Err text in the second value when it didn't, with the switch on).
fn point_and_play_with(
    core: &Core,
    client: &str,
    realm: &str,
    start: bool,
    session: Option<crate::hybrid::Session>,
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
                .map(|c| format!("{} runs {}: pick {} for it.", r.name, c.build.version(), c.name))
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
    clients::write_realmlist(&profile, &realm).map_err(|e| format!("Could not write realmlist.wtf: {e}"))?;
    let mut hybrid: Result<(), String> = Err(String::new());
    if !start {
        return Ok((format!("{} now points at {realm}.", profile.name), hybrid));
    }
    crate::gfx::prepare_launch(&profile)?;
    let hybrid_on = core.snapshot().hybrid_login && profile.build == clients::Build::V335a;
    match &session {
        Some(s) if hybrid_on => hybrid = crate::hybrid::write(&profile.path, Some(s)).map(|_| ()),
        Some(_) => {}
        None => {
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
    }
    let plan = launch::plan(&profile, &wine, launch::Os::current())?;
    let child = launch::run(&plan).inspect_err(|_| {
        let _ = crate::hybrid::scrub(&profile.path);
    })?;
    if let Ok(mut g) = core.games.lock() {
        g.push(child);
    }
    if hybrid.is_ok() {
        scrub_when_closed(profile.path.clone());
    }
    Ok((format!("Started {} on {realm}.{ground_note}", profile.name), hybrid))
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

/// Point the client at the realm, then start it.
#[tauri::command]
fn play(core: State<Core>, client: String, realm: String) -> Result<String, String> {
    point_and_play(&core, &client, &realm, true)
}

/// Play on a realm from the Realms page: the official realm or a listed Beacon (its relay address). `client` is
/// the player's pick; when its build does not fit the realm, one of their clients that fits is used, or the
/// reply says which build is missing.
#[tauri::command]
async fn play_realm(core: State<'_, Core>, client: String, slug: String) -> Result<String, String> {
    let dir = core.dir.clone();
    let realm = realms::cached(&dir).into_iter().find(|r| r.slug == slug).ok_or("That realm is not on the list any more.")?;
    let client = {
        let s = core.snapshot();
        realms::pick_client(&realm, &s.clients, Some(&client))?.id.clone()
    };
    let r2 = realm.clone();
    let addr = blocking(move || realms::desktop_address(&r2, |relay| format!("https://{relay}"))).await?;
    let mut msg = point_and_play(&core, &client, &addr, true)?;
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
        if e == "signed_out" { "Sign in on Home first (top right: your Hero Key or Google, on the web).".to_string() } else { e }
    })?;
    let profile = core.client(&client)?;
    let _ = crate::herokey::set_account_name(&profile.path, &login.user);
    let mut login = login;
    let r2 = realm.clone();
    let addr = blocking(move || realms::desktop_address(&r2, |relay| format!("https://{relay}"))).await?;
    // The hybrid client (Settings' hybrid_login, docs/HYBRID-CLIENT.md): the login screen signs in by itself.
    let s = crate::hybrid::Session { user: login.user.clone(), pass: login.pass.clone(), realm: None, enter: false };
    let (_, hybrid) = point_and_play_with(&core, &client, &addr, true, Some(s))?;
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
        return Ok("The game signs in by itself on Hero Key realms.".into());
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
}

/// A private realm's invite link: open its door for this PC (the relay's knock), then point the client at it.
#[tauri::command]
async fn join_invite(core: State<'_, Core>, client: String, link: String) -> Result<Joined, String> {
    core.client(&client)?;
    let (relay, rid, secret) = realms::parse_invite(&link).ok_or("That is not an invite link (https://<relay>/join/<realm>#<code>).")?;
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
    Ok(Joined { address, name })
}

/// Your Den on Den Lite (host/lite.rs): start it when it isn't running, wait for its login port (up to 15 s), put its
/// account in the client's Config.wtf, point the client at it and start the game.
#[tauri::command]
async fn den_play_lite(core: State<'_, Core>, client: String) -> Result<String, String> {
    use crate::host::lite::{self, DenLite};
    let profile = core.client(&client)?;
    if profile.build != crate::clients::Build::V335a {
        return Err("Your Den plays with a 3.3.5a game folder.".into());
    }
    let game = profile.path.clone();
    blocking(move || {
        let l = lite::Binary::find(&crate::settings::app_dir()).with_client(&game);
        l.start()?;
        for _ in 0..50 {
            if l.status().running {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        Err("Den Lite started but isn't answering yet. Try again in a moment.".to_string())
    })
    .await?;
    let _ = crate::herokey::set_account_name(&profile.path, lite::ACCOUNT);
    point_and_play(&core, &client, lite::ADDRESS, true)
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
        None => (status::host_port(&realm).0 == OFFICIAL_REALM).then(|| status::OFFICIAL_STATUS_URL.to_string()),
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
    up.install(&bytes).map_err(|e| format!("The update could not be installed: {e}"))?;
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

/// Check at start, then every 6 hours, for as long as Leyline runs. An update the last run downloaded and left
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            den_prefill_account,
            den_join_page,
            forge_state, // Forge, the Den's terrain editor (host/forge.rs, ui/den.js "Open in Forge")
            forge_install,
            forge_open,
            crate::p2p::cmds::host_den_open,
            crate::p2p::cmds::host_modes, // modes anyone can host and fork: cards, fork, change, remove, host one (host/modes.rs)
            crate::p2p::cmds::host_mode_fork,
            crate::p2p::cmds::host_mode_save,
            crate::p2p::cmds::host_mode_remove,
            crate::p2p::cmds::host_mode_use,
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
            crate::social::commands::social_new_account,
            crate::social::commands::social_password_signin,
            crate::social::commands::save_text_file,
            // the Forum page: reading the website's forum, opening its pages (forum.rs)
            crate::forum::commands::forum_call,
            crate::bugs::commands::bug_report,
            crate::forum::commands::forum_post,
            crate::forum::commands::forum_open,
            // List your mod: check, list, report, edit, remove with the launcher's sign-in (listing.rs)
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
