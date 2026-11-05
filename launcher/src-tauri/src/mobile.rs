//! The phone app (Android; docs/ANDROID.md): the same UI as the desktop launcher (ui/), laid out for a phone
//! (ui/platform.js, ui/phone.css), over a short list of commands.
//!
//! On a phone Leyline plays the browser game inside the app (ui/phone.js opens the website's Play page full screen),
//! signs in with the Hero Key, lists the realms, lets you browse mods, loadouts, looks and game modes, reads the forum
//! and the news, and opens links. Everything that needs a computer is left out here, so the phone build never
//! compiles it: game folders, launching the native game, hosting realms, Your Den and Den Lite, Docker, patch
//! writing, mod installs, Project Reforged, Be a node. The UI hides those features with the `platform` command.
//!
//! The commands keep the desktop names and answers (app.rs), so the UI needs no second version of itself.

use crate::catalog::{self, Catalog};
use crate::realms::{self, RealmList};
use crate::settings::{self, Settings};
use crate::{clients, net, status, BROWSER_PLAY_URL, OFFICIAL_REALM};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// The phone app's state: its own data folder (inside the app's sandbox) and the addresses it may open.
struct Phone {
    dir: PathBuf,
    /// Addresses the core gave the UI (a mod's page, a realm's Discord), which `open_url` may open.
    known_urls: Mutex<HashSet<String>>,
}

impl Phone {
    fn allow(&self, urls: impl IntoIterator<Item = String>) {
        if let Ok(mut k) = self.known_urls.lock() {
            for u in urls {
                if net::plain_https(&u) && k.len() < 20_000 {
                    k.insert(u);
                }
            }
        }
    }
}

fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
    async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())? }
}

/// What the UI runs on (ui/platform.js): a phone.
#[tauri::command]
fn platform() -> crate::Platform {
    crate::platform()
}

/// The desktop `get_state`'s answer, with no game folders (a phone has none) and `platform`.
#[derive(Serialize)]
struct AppState {
    version: &'static str,
    settings: Settings,
    official_realm: &'static str,
    browser_url: &'static str,
    windows: bool,
    data_dir: String,
    platform: crate::Platform,
}

#[tauri::command]
fn get_state(phone: State<Phone>) -> AppState {
    let mut s = settings::load(&phone.dir);
    s.clients.clear();
    AppState {
        version: env!("CARGO_PKG_VERSION"),
        settings: s,
        official_realm: OFFICIAL_REALM,
        browser_url: BROWSER_PLAY_URL,
        windows: false,
        data_dir: phone.dir.display().to_string(),
        platform: crate::platform(),
    }
}

/// The realm list (Official and Community), as on the desktop.
#[tauri::command]
async fn realms_list(phone: State<'_, Phone>) -> Result<RealmList, String> {
    let dir = phone.dir.clone();
    let l = blocking(move || Ok(realms::fetch_all(&dir))).await?;
    phone.allow(l.realms.iter().flat_map(|r| [r.join_url.clone(), r.discord.clone(), r.report_url.clone()]).flatten());
    Ok(l)
}

/// A realm's status file (the official one, or a relay realm's own). No login-port ping is needed for it to answer.
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

/// The Workshop catalog: browsing only on a phone (installs need the game on a computer).
#[tauri::command]
async fn mods_catalog(phone: State<'_, Phone>) -> Result<Catalog, String> {
    let dir = phone.dir.clone();
    let c = blocking(move || catalog::fetch(&dir)).await?;
    phone.allow(c.projects.iter().flat_map(|p| [p.repo.clone(), p.page.clone(), p.author_url.clone()]).filter(|u| !u.is_empty()));
    Ok(c)
}

/// Discover's sort tabs (no game folder on a phone, so no build filter).
#[tauri::command]
async fn mods_ranked(
    phone: State<'_, Phone>,
    sort: String,
    kind: String,
    client: Option<String>,
    refresh: bool,
) -> Result<crate::rankings::Ranked, String> {
    let _ = client;
    let dir = phone.dir.clone();
    let kind = if matches!(kind.as_str(), "addon" | "mod" | "mode" | "theme") { kind } else { String::new() };
    let (metrics, from_cache, error) = blocking(move || {
        let have = crate::rankings::cached(&dir);
        Ok(if refresh || have.is_empty() { crate::rankings::fetch(&dir, &crate::rankings::site()) } else { (have, true, None) })
    })
    .await?;
    let cat = catalog::cached(&phone.dir, catalog::URL).unwrap_or_default();
    let ids = crate::rankings::rank(&cat, &metrics, crate::rankings::Sort::parse(&sort), &kind, None);
    Ok(crate::rankings::Ranked { ids, metrics, from_cache, error })
}

/// The website's Play page in the phone's browser (Chrome): the way in when this phone's web view can't draw the game
/// (ui/phone.js, the WebGPU check). The UI can't pass an address.
#[tauri::command]
fn open_browser_play(app: AppHandle) -> Result<(), String> {
    app.opener().open_url(BROWSER_PLAY_URL, None::<&str>).map_err(|e| e.to_string())
}

/// One of the launcher's fixed pages (`crate::LINKS`) by name, in the phone's browser.
#[tauri::command]
fn open_link(app: AppHandle, name: String) -> Result<(), String> {
    let url = crate::link(&name).ok_or("Unknown link.")?;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// An address the core handed the UI (a mod's page, a realm's Discord). Anything else is refused.
#[tauri::command]
fn open_url(app: AppHandle, phone: State<Phone>, url: String) -> Result<(), String> {
    let ok = phone.known_urls.lock().map(|k| k.contains(&url)).unwrap_or(false);
    if !ok || !net::plain_https(&url) {
        return Err("Leyline only opens the pages it listed itself.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// A `leyline://` link Leyline was started with: none on a phone (no link scheme is registered there yet).
#[tauri::command]
fn deep_link_take() -> Option<serde_json::Value> {
    None
}

/// The phone app's entry point (Tauri's Android activity calls it).
#[tauri::mobile_entry_point]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // The app's own data folder (inside its sandbox; no storage permission): the settings, the sign-in and the
            // cached listings. Everything that calls settings::app_dir() finds it through the same override.
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::env::set_var(settings::APP_DIR_ENV, &dir);
            app.manage(Phone { dir, known_urls: Mutex::new(HashSet::new()) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platform,
            get_state,
            realms_list,
            realm_status,
            mods_catalog,
            mods_ranked,
            open_browser_play,
            open_link,
            open_url,
            deep_link_take,
            // the Hero Key sign-in (gate.js, social.js): the same commands as the desktop
            crate::social::commands::social_status,
            crate::social::commands::social_signin_start,
            crate::social::commands::social_signin_reopen,
            crate::social::commands::social_signin_poll,
            crate::social::commands::social_signin_cancel,
            crate::social::commands::social_call,
            crate::social::commands::social_ways,
            crate::social::commands::social_key_signin,
            crate::social::commands::social_key_file,
            crate::social::commands::social_key_found,
            crate::social::commands::social_new_account,
            crate::social::commands::social_password_signin,
            crate::social::commands::social_signout,
            // the Forum and News pages, and the game modes' browser links
            crate::forum::commands::forum_call,
            crate::forum::commands::forum_post,
            crate::forum::commands::forum_open,
            crate::news::commands::news_list,
            crate::modes::commands::modes_list,
            crate::modes::commands::mode_open_browser,
            crate::modes::commands::mode_open_credit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Leyline");
}
