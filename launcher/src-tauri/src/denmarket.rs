//! Shared Dens from the launcher (`ui/denmarket.js`, docs/DEN-MARKETPLACE.md): Mods & Addons › Dens, the catalogue
//! of Dens and stamps players shared (the website's `stream/denmarket.py`, `/wrath/api/dens`), with the launcher's own
//! Leyline sign-in (`crate::social`'s token) for Share, Like and Report: no second sign-in.
//!
//! The UI names what it wants with plain parameters; it never names a URL or a file path it made up:
//! * browse, one build, mine: GET with checked parameters ([`browse_url`]).
//! * Open in my Den ([`fetch_checked`]): the build's file from the API (512 KB at most), checked against the sha256
//!   the build's page gave and like any Den file (`host::dens::check`), added to Saved Dens and opened (as Add from a
//!   link does: `host::yourden::fetch_add`).
//! * Add as a stamp ([`as_stamp`]): the same download, then into Den Lite's `<dens>/stamps-shared/<name>.den.json`
//!   (labs/den-lite den.rs SHARED_DIR): `.den stamp <name>` or "Jaina, build <name>" places it, and every piece keeps
//!   the maker's credit in the Den it lands in.
//! * Share and update ([`share_body`]): the saved Den's own text (`host::dens::text`), the words checked here and
//!   again on the server, and the pictures: the game's own screenshots from the showcase ([`showcase_request`]: Den
//!   Lite poses its far camera at four angles and the addon presses Screenshot, labs/den-lite showcase.rs), found in
//!   the game folders' `Screenshots` ([`find_shots`]), or pictures the player picked (JPEG or PNG, 900 KB each).
//!   A path the UI sends back is taken only inside a game folder's Screenshots ([`shot_ok`]), so nothing else on
//!   the PC can be sent.

use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The official API and the catalogue on the website.
pub const API_BASE: &str = crate::site!("/wrath/api/dens");
pub const PAGE: &str = crate::site!("/wrath/dens");
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// A share carries the Den and up to four pictures.
pub const SHARE_TIMEOUT: Duration = Duration::from_secs(120);
pub const UNREACHABLE: &str = "The website isn't reachable right now. Try again in a minute.";
/// Pictures (stream/denmarket.py PIC_MAX, PICS).
pub const PIC_MAX: u64 = 900 * 1024;
pub const PICS: usize = 4;
/// Stamps (labs/den-lite den.rs STAMP_MAX, STAMP_SPAN).
pub const STAMP_MAX: usize = 60;
pub const STAMP_SPAN: f64 = 40.0;
/// Den Lite's folder of stamps from the shared Dens (inside its Dens folder).
pub const SHARED_STAMPS: &str = "stamps-shared";
/// The showcase: the launcher's ask and Den Lite's answer, in Den Lite's Dens folder (labs/den-lite showcase.rs).
pub const SHOWCASE_ASK: &str = "showcase.json";
pub const SHOWCASE_DONE: &str = "showcase-done.json";
pub const KINDS: &[&str] = &["den", "stamp"];
pub const SORTS: &[&str] = &["new", "likes", "downloads"];
/// stream/denmarket.py LICENCES and REPORT_REASONS.
pub const LICENCES: &[&str] = &["cc-by", "cc-by-sa", "cc-by-nc", "cc0", "look"];
pub const REASONS: &[&str] = &["stolen", "rude", "spam", "broken", "gamedata", "other"];

/// The API base: the official one, or LEYLINE_DENS_API for a test server (https, or http on this PC only).
pub fn base() -> String {
    match std::env::var("LEYLINE_DENS_API") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => {
            b.trim_end_matches('/').to_string()
        }
        _ => API_BASE.to_string(),
    }
}

/// A shared build's id: 10 of the site's letters (no l, o, 0, 1).
pub fn id_ok(s: &str) -> bool {
    s.len() == 10 && s.bytes().all(|b| b"abcdefghijkmnpqrstuvwxyz23456789".contains(&b))
}

fn word(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn enc(s: &str) -> String {
    s.bytes().map(|c| if c.is_ascii_alphanumeric() || b"-_.~".contains(&c) { (c as char).to_string() } else { format!("%{c:02X}") }).collect()
}

fn text_ok(s: &str, max: usize) -> bool {
    s.chars().count() <= max && !s.chars().any(|c| c.is_control() && c != '\n')
}

/// The catalogue's address with the UI's filters, each checked: q (80), kind, tag, sort, page, by (a player's id).
pub fn browse_url(base: &str, q: &Value) -> Result<String, String> {
    let mut parts = Vec::new();
    let s = |k: &str| q.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
    if let Some(v) = s("q") {
        if !text_ok(v, 80) || v.contains('\n') {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("q={}", enc(v)));
    }
    if let Some(v) = s("kind") {
        if !KINDS.contains(&v) {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("kind={v}"));
    }
    if let Some(v) = s("tag") {
        if !word(v, 16) {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("tag={v}"));
    }
    if let Some(v) = s("sort") {
        if !SORTS.contains(&v) {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("sort={v}"));
    }
    if let Some(v) = s("by") {
        if !id_ok(v) {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("by={v}"));
    }
    if let Some(n) = q.get("page").and_then(Value::as_u64) {
        if !(1..=1000).contains(&n) {
            return Err("Bad parameter.".into());
        }
        parts.push(format!("page={n}"));
    }
    Ok(if parts.is_empty() { base.to_string() } else { format!("{base}?{}", parts.join("&")) })
}

/// What the UI may post besides share and update: (action, the parameters it may send).
pub const POSTS: &[(&str, &[&str])] = &[("like", &["id", "on"]), ("report", &["id", "reason", "note"]), ("unpublish", &["id"])];

/// The JSON body for like, report or unpublish: unknown actions refused, unknown parameters dropped, each checked.
pub fn post_for(action: &str, args: &Value) -> Result<Value, String> {
    let (_, keys) = POSTS.iter().find(|a| a.0 == action).ok_or("Unknown action.")?;
    let mut body = Map::new();
    for k in keys.iter() {
        let Some(v) = args.get(*k).filter(|v| !v.is_null()) else { continue };
        let ok = match (*k, v) {
            ("id", Value::String(s)) => id_ok(s),
            ("on", Value::Bool(_)) => true,
            ("reason", Value::String(s)) => REASONS.contains(&s.as_str()),
            ("note", Value::String(s)) => text_ok(s, 300),
            _ => false,
        };
        if !ok {
            return Err("Bad parameter.".into());
        }
        body.insert((*k).to_string(), v.clone());
    }
    if !body.contains_key("id") {
        return Err("Which build?".into());
    }
    Ok(Value::Object(body))
}

/// The words of a share or an update, checked: kind, title (3 to 48), description (600), credit (80), tags (up to 5
/// words), licence, based_on and id (an update). The server checks them all again.
pub fn meta_for(args: &Value, update: bool) -> Result<Map<String, Value>, String> {
    let mut m = Map::new();
    let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::trim);
    if update {
        let id = s("id").filter(|v| id_ok(v)).ok_or("Which build?")?;
        m.insert("id".into(), json!(id));
    } else {
        let kind = s("kind").unwrap_or("den");
        if !KINDS.contains(&kind) {
            return Err("Share a Den or a stamp.".into());
        }
        m.insert("kind".into(), json!(kind));
    }
    if let Some(t) = s("title").or(if update { None } else { Some("") }) {
        if t.chars().count() < 3 || !text_ok(t, 48) || t.contains('\n') {
            return Err("Give it a title (3 to 48 letters).".into());
        }
        m.insert("title".into(), json!(t));
    }
    for (k, max) in [("description", 600), ("credit", 80)] {
        if let Some(v) = s(k) {
            if !text_ok(v, max) || (k == "credit" && v.contains('\n')) {
                return Err(format!("The {k} is too long."));
            }
            m.insert(k.into(), json!(v));
        }
    }
    if let Some(t) = args.get("tags").filter(|v| !v.is_null()) {
        let tags: Vec<&str> = t.as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if tags.len() > 5 || t.as_array().map_or(true, |a| a.len() != tags.len()) || !tags.iter().all(|x| word(x, 16)) {
            return Err("Pick up to 5 tags.".into());
        }
        m.insert("tags".into(), json!(tags));
    }
    if let Some(l) = s("licence") {
        if !LICENCES.contains(&l) {
            return Err("Pick a licence from the list.".into());
        }
        m.insert("licence".into(), json!(l));
    }
    if let Some(b) = s("based_on").filter(|v| !v.is_empty()) {
        if !id_ok(b) {
            return Err("Bad parameter.".into());
        }
        m.insert("based_on".into(), json!(b));
    }
    Ok(m)
}

/// JPEG or PNG, 900 KB at most (the server reads the size again).
pub fn picture_ok(b: &[u8]) -> bool {
    !b.is_empty() && b.len() as u64 <= PIC_MAX && (b.starts_with(b"\xff\xd8\xff") || b.starts_with(b"\x89PNG\r\n\x1a\n"))
}

/// A screenshot path the UI sent back: a .jpg or .png directly in one of the game folders' `Screenshots`.
pub fn shot_ok(path: &Path, game_folders: &[PathBuf]) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if !matches!(ext.as_deref(), Some("jpg") | Some("jpeg") | Some("png")) || path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return false;
    }
    let Some(parent) = path.parent() else { return false };
    game_folders.iter().any(|g| parent == g.join("Screenshots"))
}

/// The newest screenshots (up to four, newest first) the game took since `since` in the game folders' Screenshots:
/// what the showcase made. Big or odd files are left out.
pub fn find_shots(game_folders: &[PathBuf], since: SystemTime) -> Vec<PathBuf> {
    let mut got: Vec<(SystemTime, PathBuf)> = Vec::new();
    for g in game_folders {
        let Ok(rd) = fs::read_dir(g.join("Screenshots")) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(m) = e.metadata() else { continue };
            let Ok(t) = m.modified() else { continue };
            if m.is_file() && t >= since && m.len() <= PIC_MAX && shot_ok(&p, game_folders) {
                got.push((t, p));
            }
        }
    }
    got.sort_by(|a, b| b.0.cmp(&a.0));
    got.into_iter().take(PICS).map(|x| x.1).collect()
}

/// The share's or update's body: the words, the Den's text (when given) and the pictures (paths checked with
/// [`shot_ok`], then base64 from the player's own picks), each checked with [`picture_ok`].
pub fn share_body(meta: Map<String, Value>, den: Option<String>, shots: &[PathBuf], picked: &[String], game_folders: &[PathBuf]) -> Result<Value, String> {
    let mut body = meta;
    if let Some(t) = den {
        body.insert("den".into(), json!(t));
    }
    let mut pics = Vec::new();
    for p in shots {
        if !shot_ok(p, game_folders) {
            return Err("That picture isn't one of your game's screenshots.".into());
        }
        let b = fs::read(p).map_err(|_| "A screenshot couldn't be read.".to_string())?;
        if !picture_ok(&b) {
            return Err("A screenshot is too big or isn't a JPEG or PNG (900 KB at most).".into());
        }
        pics.push(crate::themes::base64(&b));
    }
    for s in picked {
        let b = crate::update::b64decode(s).ok_or("That picture didn't arrive whole.")?;
        if !picture_ok(&b) {
            return Err("Pictures are JPEG or PNG, 900 KB at most.".into());
        }
        pics.push(crate::themes::base64(&b));
    }
    if pics.len() > PICS {
        return Err(format!("Up to {PICS} pictures."));
    }
    if !shots.is_empty() || !picked.is_empty() {
        body.insert("pictures".into(), json!(pics));
    }
    Ok(Value::Object(body))
}

/// A downloaded build: the sha256 its page gave, then the Den file checks. Returns the text.
pub fn check_download(text: &str, sha_got: &str, sha_want: &str) -> Result<String, String> {
    if sha_want.len() != 64 || !sha_got.eq_ignore_ascii_case(sha_want) {
        return Err("That Den changed while it was downloading: try again.".into());
    }
    crate::host::dens::check(text)?;
    Ok(text.to_string())
}

/// The Saved Dens name for a shared build: its title in plain letters (24 at most), else "shared-<id>".
pub fn file_name_for(title: &str, id: &str) -> String {
    let mut s: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s: String = s.trim_matches('-').chars().take(24).collect::<String>().trim_end_matches('-').to_string();
    crate::host::dens::clean_name(&s).unwrap_or_else(|| format!("shared-{id}"))
}

/// A shared build as a stamp: up to 60 pieces within 40 yards, "kind": "stamp", its title; its credit kept.
pub fn as_stamp(text: &str, title: &str) -> Result<String, String> {
    let mut doc: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "That isn't a Den file.".to_string())?;
    let props = doc.get("props").and_then(Value::as_array).ok_or("That isn't a Den file.")?;
    let near = |p: &Value| ["x", "y", "z"].iter().all(|k| p.get(*k).and_then(Value::as_f64).unwrap_or(0.0).abs() <= STAMP_SPAN);
    if props.is_empty() || props.len() > STAMP_MAX || !props.iter().all(near) {
        return Err(format!("That one is too big for a stamp (up to {STAMP_MAX} pieces within {} yards): open it as a Den instead.", STAMP_SPAN as u32));
    }
    let o = doc.as_object_mut().ok_or("That isn't a Den file.")?;
    o.insert("kind".into(), json!("stamp"));
    o.insert("title".into(), json!(title.chars().filter(|c| !c.is_control()).take(48).collect::<String>()));
    o.remove("room");
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// Writes the stamp into Den Lite's shared stamps (never over another one: "camp-2"). Returns its name.
pub fn add_stamp(dens_dir: &Path, name: &str, text: &str) -> Result<String, String> {
    let dir = dens_dir.join(SHARED_STAMPS);
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the stamps folder: {e}"))?;
    let base = crate::host::dens::clean_name(name).unwrap_or_else(|| "stamp".into());
    for i in 1..100 {
        let n = if i == 1 { base.clone() } else { format!("{}-{i}", &base[..base.len().min(21)]) };
        let p = dir.join(format!("{n}.den.json"));
        if p.exists() && fs::read_to_string(&p).ok().as_deref() == Some(text) {
            return Ok(n);   // the same stamp again
        }
        if !p.exists() {
            fs::write(&p, text).map_err(|e| format!("Couldn't save the stamp: {e}"))?;
            return Ok(n);
        }
    }
    Err("Too many stamps with that name.".into())
}

/// The showcase's ask for Den Lite: `{"id", "file"}` (the Den to open and pose first; "" for the one you're in).
pub fn showcase_request(dens_dir: &Path, file: &str, id: &str) -> Result<(), String> {
    let file = if file.is_empty() { String::new() } else { crate::host::dens::clean_name(file).ok_or("That isn't a Den file's name.")? };
    fs::create_dir_all(dens_dir).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(dens_dir.join(SHOWCASE_DONE));
    fs::write(dens_dir.join(SHOWCASE_ASK), json!({ "id": id, "file": file }).to_string()).map_err(|e| format!("Couldn't ask your Den: {e}"))
}

/// Den Lite's answer to the ask `id`: Some(pictures it asked the game for), or None while it isn't done.
pub fn showcase_done(dens_dir: &Path, id: &str) -> Option<Value> {
    let p = dens_dir.join(SHOWCASE_DONE);
    if fs::metadata(&p).ok()?.len() > 4096 {
        return None;
    }
    let v: Value = serde_json::from_str(&fs::read_to_string(&p).ok()?).ok()?;
    (v.get("id").and_then(Value::as_str) == Some(id)).then_some(v)
}

fn friendly(e: String) -> String {
    if e.starts_with("Can't reach") || e.starts_with("can't reach") {
        UNREACHABLE.to_string()
    } else {
        e
    }
}

#[cfg(all(feature = "app", desktop))] // desktop only: the phone app has its own short list (mobile.rs)
pub mod commands {
    use super::*;
    use crate::{host, settings, social};
    use tauri::AppHandle;
    use tauri_plugin_opener::OpenerExt;

    fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> impl std::future::Future<Output = Result<T, String>> {
        async move { tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())? }
    }

    fn game_folders() -> Vec<PathBuf> {
        settings::load(&settings::app_dir()).clients.into_iter().map(|c| c.path).collect()
    }

    fn lite_dens() -> PathBuf {
        host::dens::dir(&host::lite::root(&settings::app_dir()))
    }

    /// GET as the signed-in player when there is one (mine, liked), else as anyone.
    fn get(url: &str) -> Result<Value, String> {
        match social::call_as_player(&settings::app_dir(), url, None, TIMEOUT) {
            Err(e) if e == "signed_out" => crate::net::get_json(url, TIMEOUT).map_err(friendly),
            r => r.map_err(friendly),
        }
    }

    fn detail(id: &str) -> Result<Value, String> {
        if !id_ok(id) {
            return Err("No such build.".into());
        }
        let v = get(&format!("{}/{id}", base()))?;
        v.get("build").cloned().filter(Value::is_object).ok_or_else(|| "The server's answer didn't look right.".into())
    }

    /// The build's file, checked against its page's sha256 and as a Den file: (the build, the text).
    fn fetch_checked(id: &str) -> Result<(Value, String), String> {
        let b = detail(id)?;
        let want = b.get("sha256").and_then(Value::as_str).unwrap_or("").to_string();
        let scratch = settings::app_dir().join("tmp");
        fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
        let tmp = scratch.join(format!("den-shared-{}-{id}.json", std::process::id()));
        let got = crate::net::download(&format!("{}/{id}/file", base()), &tmp, host::dens::MAX_BYTES, TIMEOUT, TIMEOUT);
        let text = got.and_then(|(_, sha)| fs::read_to_string(&tmp).map(|t| (t, sha)).map_err(|_| "That isn't a Den file.".to_string()));
        let _ = fs::remove_file(&tmp);
        let (text, sha) = text.map_err(friendly)?;
        Ok((b, check_download(&text, &sha, &want)?))
    }

    /// The catalogue: featured, items, tags, the licences and reasons (the forms' lists).
    #[tauri::command]
    pub async fn dens_browse(query: Option<Value>) -> Result<Value, String> {
        let q = query.unwrap_or(Value::Null);
        blocking(move || crate::net::get_json(&browse_url(&base(), &q)?, TIMEOUT).map_err(friendly)).await
    }

    /// One build (pictures, lineage, licence, sha256; liked and mine when signed in).
    #[tauri::command]
    pub async fn dens_detail(id: String) -> Result<Value, String> {
        blocking(move || detail(&id)).await
    }

    /// The player's own shared builds, waiting ones too.
    #[tauri::command]
    pub async fn dens_mine() -> Result<Value, String> {
        blocking(|| social::call_as_player(&settings::app_dir(), &format!("{}/mine", base()), None, TIMEOUT).map_err(friendly)).await
    }

    /// Like, report or unpublish as the signed-in player.
    #[tauri::command]
    pub async fn dens_post(action: String, args: Option<Value>) -> Result<Value, String> {
        let args = args.unwrap_or(Value::Null);
        blocking(move || {
            let body = post_for(&action, &args)?;
            social::call_as_player(&settings::app_dir(), &format!("{}/{action}", base()), Some(&body), TIMEOUT).map_err(friendly)
        })
        .await
    }

    /// Share a saved Den (`name`) or update a shared one (`args.id`; `name` only to send a new version of the file):
    /// the words in `args`, the showcase's screenshots (`shots`) and pictures the player picked (`pictures`, base64).
    #[tauri::command]
    pub async fn dens_share(name: Option<String>, args: Value, shots: Option<Vec<String>>, pictures: Option<Vec<String>>) -> Result<Value, String> {
        blocking(move || {
            let update = args.get("id").is_some();
            let meta = meta_for(&args, update)?;
            let den = match name.as_deref().filter(|n| !n.is_empty()) {
                Some(n) => Some(host::dens::text(&crate::p2p::cmds::den_root(), n)?),
                None if update => None,
                None => return Err("Which saved Den?".into()),
            };
            let shots: Vec<PathBuf> = shots.unwrap_or_default().into_iter().map(PathBuf::from).collect();
            let body = share_body(meta, den, &shots, &pictures.unwrap_or_default(), &game_folders())?;
            let action = if update { "update" } else { "share" };
            social::call_as_player(&settings::app_dir(), &format!("{}/{action}", base()), Some(&body), SHARE_TIMEOUT).map_err(friendly)
        })
        .await
    }

    /// Open in my Den: the build downloaded and checked, into Saved Dens, opened when you're in your Den.
    #[tauri::command]
    pub async fn dens_open(id: String) -> Result<Value, String> {
        blocking(move || {
            let (b, text) = fetch_checked(&id)?;
            let root = crate::p2p::cmds::den_root();
            let title = b.get("title").and_then(Value::as_str).unwrap_or("");
            let name = host::dens::add(&root, &format!("{}.den.json", file_name_for(title, &id)), &text)?;
            host::dens::open(&root, &name)?;
            Ok(json!({ "name": name, "title": title }))
        })
        .await
    }

    /// Add as a stamp: the build into Den Lite's shared stamps; `.den stamp <name>` places it.
    #[tauri::command]
    pub async fn dens_stamp(id: String) -> Result<Value, String> {
        blocking(move || {
            let (b, text) = fetch_checked(&id)?;
            let title = b.get("title").and_then(Value::as_str).unwrap_or("").to_string();
            let name = add_stamp(&lite_dens(), &file_name_for(&title, &id), &as_stamp(&text, &title)?)?;
            Ok(json!({ "name": name, "title": title }))
        })
        .await
    }

    /// The showcase: Den Lite opens `name` (or keeps the Den you're in), poses the far camera at four angles and the
    /// game takes a screenshot at each. Returns the ask's id and its start (ms), for dens_shots.
    #[tauri::command]
    pub async fn dens_showcase(name: Option<String>) -> Result<Value, String> {
        blocking(move || {
            let since = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
            let id = format!("sc{since:x}");
            showcase_request(&lite_dens(), name.as_deref().unwrap_or(""), &id)?;
            Ok(json!({ "id": id, "since": since }))
        })
        .await
    }

    /// The showcase's pictures so far: {done, shots: [{path, name, bytes, data}]} (data: base64, for the preview).
    #[tauri::command]
    pub async fn dens_shots(id: String, since: u64) -> Result<Value, String> {
        blocking(move || {
            let t = SystemTime::UNIX_EPOCH + Duration::from_millis(since.saturating_sub(2000));
            let done = showcase_done(&lite_dens(), &id);
            let shots: Vec<Value> = find_shots(&game_folders(), t)
                .into_iter()
                .filter_map(|p| {
                    let b = fs::read(&p).ok().filter(|b| picture_ok(b))?;
                    Some(json!({ "path": p.display().to_string(), "name": p.file_name()?.to_string_lossy(), "bytes": b.len(),
                        "data": crate::themes::base64(&b), "png": b.starts_with(b"\x89PNG") }))
                })
                .collect();
            Ok(json!({ "done": done.is_some(), "answer": done, "shots": shots }))
        })
        .await
    }

    /// The catalogue (or one build's showcase) on the website.
    #[tauri::command]
    pub fn dens_page_open(app: AppHandle, id: Option<String>) -> Result<(), String> {
        let url = match id.filter(|i| id_ok(i)) {
            Some(i) => format!("{PAGE}/{i}"),
            None => PAGE.to_string(),
        };
        app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEN: &str = r#"{"format":"leyline-den","version":1,"name":"camp","props":[{"entry":1,"x":2,"y":0},{"entry":2,"x":-3,"y":5,"credit":0}],
        "credits":[{"id":"abcdefghij","title":"Camp","by":"Bea"}],"credit":{"id":"abcdefghij","by":"Bea","made_with":"Made with Leyline"},"room":{"look":"white","size":16}}"#;

    #[test]
    fn ids_and_browse_parameters_are_checked() {
        assert!(id_ok("abcdefghij") && id_ok("z2z3z4z5z6"));
        for bad in ["abcdefghi", "abcdefghijk", "abcdefghil", "ABCDEFGHIJ", "../../../x", "abcdefgh0j"] {
            assert!(!id_ok(bad), "{bad}");
        }
        let b = "https://x/api";
        assert_eq!(browse_url(b, &Value::Null).unwrap(), b);
        assert_eq!(browse_url(b, &json!({"q": "cozy inn & fire", "kind": "stamp", "tag": "inn", "sort": "likes", "page": 2})).unwrap(),
            "https://x/api?q=cozy%20inn%20%26%20fire&kind=stamp&tag=inn&sort=likes&page=2");
        for bad in [json!({"kind": "mod"}), json!({"sort": "random"}), json!({"tag": "Inn!"}), json!({"page": 0}), json!({"by": "nope"}),
            json!({"q": "x".repeat(81)}), json!({"q": "a\u{7}"})] {
            assert!(browse_url(b, &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn posts_send_only_their_fields() {
        assert_eq!(post_for("like", &json!({"id": "abcdefghij", "on": false, "x": 1})).unwrap(), json!({"id": "abcdefghij", "on": false}));
        assert!(post_for("report", &json!({"id": "abcdefghij", "reason": "stolen", "note": "mine"})).is_ok());
        assert!(post_for("report", &json!({"id": "abcdefghij", "reason": "because"})).is_err());
        assert!(post_for("unpublish", &json!({"id": "../x"})).is_err());
        assert!(post_for("unpublish", &json!({})).is_err(), "an id is needed");
        assert!(post_for("share", &json!({"id": "abcdefghij"})).is_err(), "share goes through dens_share");
        assert!(post_for("delete-all", &json!({})).is_err());
    }

    #[test]
    fn share_words_are_checked() {
        let m = meta_for(&json!({"title": " Bea's Garden ", "kind": "stamp", "tags": ["garden", "cozy"], "licence": "cc0",
            "description": "Rows.\n\nA bench.", "credit": "Bea", "based_on": "abcdefghij", "junk": 1}), false).unwrap();
        assert_eq!(m["title"], "Bea's Garden");
        assert_eq!(m["kind"], "stamp");
        assert!(!m.contains_key("junk"));
        assert!(meta_for(&json!({"title": "ab"}), false).is_err());
        assert!(meta_for(&json!({}), false).is_err(), "a share needs a title");
        assert!(meta_for(&json!({"title": "Good one", "kind": "mod"}), false).is_err());
        assert!(meta_for(&json!({"title": "Good one", "tags": ["a", "b", "c", "d", "e", "f"]}), false).is_err());
        assert!(meta_for(&json!({"title": "Good one", "tags": ["Garden!"]}), false).is_err());
        assert!(meta_for(&json!({"title": "Good one", "licence": "mine"}), false).is_err());
        assert!(meta_for(&json!({"title": "Good one", "credit": "a\nb"}), false).is_err());
        let u = meta_for(&json!({"id": "abcdefghij", "description": "New words."}), true).unwrap();
        assert_eq!((u["id"].as_str(), u.contains_key("title")), (Some("abcdefghij"), false), "an update keeps what it doesn't send");
        assert!(meta_for(&json!({"description": "x"}), true).is_err(), "an update needs its id");
    }

    #[test]
    fn only_the_games_own_screenshots_go_up() {
        let g = tempfile::tempdir().unwrap();
        let shots = g.path().join("Screenshots");
        fs::create_dir_all(&shots).unwrap();
        let folders = vec![g.path().to_path_buf()];
        let jpg = [b"\xff\xd8\xff\xe0".as_slice(), &[0u8; 64]].concat();
        fs::write(shots.join("WoWScrnShot_110426_101010.jpg"), &jpg).unwrap();
        fs::write(shots.join("notes.txt"), "hi").unwrap();
        fs::write(shots.join("big.jpg"), vec![0xffu8; (PIC_MAX + 1) as usize]).unwrap();
        fs::write(g.path().join("WTF.jpg"), &jpg).unwrap();
        assert!(shot_ok(&shots.join("WoWScrnShot_110426_101010.jpg"), &folders));
        assert!(!shot_ok(&g.path().join("WTF.jpg"), &folders), "not in Screenshots");
        assert!(!shot_ok(&shots.join("notes.txt"), &folders));
        assert!(!shot_ok(&shots.join("..").join("Screenshots").join("x.jpg"), &folders));
        assert!(!shot_ok(Path::new("/etc/passwd.jpg"), &folders));
        let found = find_shots(&folders, SystemTime::UNIX_EPOCH);
        assert_eq!(found, vec![shots.join("WoWScrnShot_110426_101010.jpg")], "big and odd files left out");
        assert!(find_shots(&folders, SystemTime::now() + Duration::from_secs(60)).is_empty(), "only since the showcase began");
        let meta = meta_for(&json!({"title": "Garden"}), false).unwrap();
        let b = share_body(meta.clone(), Some(DEN.into()), &found, &[crate::themes::base64(&jpg)], &folders).unwrap();
        assert_eq!(b["pictures"].as_array().unwrap().len(), 2);
        assert_eq!(b["den"], DEN);
        assert!(share_body(meta.clone(), None, &[g.path().join("WTF.jpg")], &[], &folders).is_err());
        assert!(share_body(meta.clone(), None, &[], &[crate::themes::base64(b"GIF89a")], &folders).is_err());
        assert!(share_body(meta.clone(), None, &[], &vec![crate::themes::base64(&jpg); 5], &folders).is_err());
        assert!(!share_body(meta, None, &[], &[], &folders).unwrap().as_object().unwrap().contains_key("pictures"), "no pictures: none sent");
    }

    #[test]
    fn a_download_is_checked_and_named() {
        let sha = "a".repeat(64);
        assert_eq!(check_download(DEN, &sha, &sha).unwrap(), DEN);
        assert!(check_download(DEN, &sha, &"b".repeat(64)).unwrap_err().contains("changed"));
        assert!(check_download(DEN, &sha, "").is_err(), "no fingerprint, no file");
        assert!(check_download("{}", &sha, &sha).is_err(), "and a Den file");
        assert_eq!(file_name_for("Bea's Big  Garden!", "abcdefghij"), "bea-s-big-garden");
        assert_eq!(file_name_for("A very long title for a little camp", "abcdefghij"), "a-very-long-title-for-a");
        assert_eq!(file_name_for("!!!", "abcdefghij"), "shared-abcdefghij");
    }

    #[test]
    fn a_stamp_keeps_its_credit() {
        let s = as_stamp(DEN, "Bea's camp").unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!((v["kind"].as_str(), v["title"].as_str()), (Some("stamp"), Some("Bea's camp")));
        assert_eq!(v["credit"]["by"], "Bea");
        assert_eq!(v["credits"][0]["by"], "Bea");
        assert_eq!(v["props"][1]["credit"], 0);
        assert!(v.get("room").is_none());
        let big = DEN.replace(r#""x":2"#, r#""x":200"#);
        assert!(as_stamp(&big, "x").unwrap_err().contains("too big for a stamp"));
        let d = tempfile::tempdir().unwrap();
        assert_eq!(add_stamp(d.path(), "camp", &s).unwrap(), "camp");
        assert_eq!(add_stamp(d.path(), "camp", &s).unwrap(), "camp", "the same stamp again");
        assert_eq!(add_stamp(d.path(), "camp", "{}").unwrap(), "camp-2", "another one beside it");
        assert!(d.path().join(SHARED_STAMPS).join("camp.den.json").is_file());
    }

    #[test]
    fn the_showcase_ask_and_answer() {
        let d = tempfile::tempdir().unwrap();
        showcase_request(d.path(), "garden", "sc1").unwrap();
        let ask: Value = serde_json::from_str(&fs::read_to_string(d.path().join(SHOWCASE_ASK)).unwrap()).unwrap();
        assert_eq!(ask, json!({"id": "sc1", "file": "garden"}));
        assert!(showcase_request(d.path(), "../x", "sc2").is_err());
        assert_eq!(showcase_done(d.path(), "sc1"), None);
        fs::write(d.path().join(SHOWCASE_DONE), r#"{"id":"sc0","shots":4}"#).unwrap();
        assert_eq!(showcase_done(d.path(), "sc1"), None, "another ask's answer");
        fs::write(d.path().join(SHOWCASE_DONE), r#"{"id":"sc1","shots":4}"#).unwrap();
        assert_eq!(showcase_done(d.path(), "sc1").unwrap()["shots"], 4);
    }

    #[test]
    fn a_safe_base() {
        std::env::set_var("LEYLINE_DENS_API", "http://evil.example.com/api");
        assert_eq!(base(), API_BASE);
        std::env::remove_var("LEYLINE_DENS_API");
        assert!(API_BASE.starts_with("https://") && PAGE.starts_with("https://"));
    }
}
