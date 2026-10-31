//! List your mod from the launcher (`ui/listmod.js`): the website's listings API (`stream/listings.py`,
//! `/wrath/api/listings/*`) with the launcher's own Leyline sign-in (`crate::social`'s token): no second sign-in.
//!
//! The UI names an action from `POSTS` with plain parameters; it never names a URL. The site does the work: it pins
//! the link to one exact download on the author's repo, fetches it once, checks it with the same rules as the
//! launcher (`upstream-rules.json`), and answers with what one click will install. A check can take a while (it reads
//! the download), so its read timeout is longer than the other calls'.

use serde_json::Value;
use std::time::Duration;

/// The official listings API.
pub const API_BASE: &str = "https://play.openvanilla.app/wrath/api/listings";
/// The website's List your mod page (opened in the browser when signed out).
pub const PAGE: &str = "https://play.openvanilla.app/wrath/mods/list";
/// A check fetches the mod once on the server.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(90);
pub const TIMEOUT: Duration = Duration::from_secs(10);
pub const UNREACHABLE: &str = "The website isn't reachable right now. Try again in a minute.";

/// What the UI may post: (action, the parameters it may send). Strings only.
pub const POSTS: &[(&str, &[&str])] = &[
    ("check", &["url"]),
    ("submit", &["t", "category", "description", "builds"]),
    ("report", &["id", "reason", "note"]),
    ("edit", &["id", "description", "category"]),
    ("remove", &["id"]),
];
/// Report reasons (stream/projects.py REPORT_REASONS).
pub const REASONS: &[&str] = &["broken", "malware", "stolen", "gamedata", "cheat", "rude", "spam", "other"];

fn plain(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// The JSON body for a post. Unknown actions are refused, unknown parameters dropped; each parameter has its shape.
pub fn post_for(action: &str, args: &Value) -> Result<Value, String> {
    let (_, keys) = POSTS.iter().find(|a| a.0 == action).ok_or("Unknown action.")?;
    let mut body = serde_json::Map::new();
    for k in keys.iter() {
        let v = match args.get(*k) {
            Some(Value::String(s)) => s.trim().to_string(),
            None | Some(Value::Null) => continue,
            _ => return Err("Bad parameter.".into()),
        };
        let ok = match *k {
            "url" => v.starts_with("https://") && v.len() <= 400 && !v.chars().any(|c| c.is_whitespace() || c.is_control()),
            "t" => v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()),
            "id" => v.strip_prefix("link:").unwrap_or(&v).len() == 12 && v.strip_prefix("link:").unwrap_or(&v).bytes().all(|b| b.is_ascii_hexdigit()),
            "reason" => REASONS.contains(&v.as_str()),
            "category" | "builds" => plain(&v, 32),
            _ => v.chars().count() <= 400 && !v.chars().any(|c| c.is_control()),
        };
        if !ok {
            return Err("Bad parameter.".into());
        }
        body.insert((*k).to_string(), Value::String(v));
    }
    Ok(Value::Object(body))
}

/// The API base: the official one, or LEYLINE_LISTINGS_API for a test server (https, or http on this PC only).
pub fn base() -> String {
    match std::env::var("LEYLINE_LISTINGS_API") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => {
            b.trim_end_matches('/').to_string()
        }
        _ => API_BASE.to_string(),
    }
}

fn friendly(e: String) -> String {
    if e.starts_with("Can't reach") {
        UNREACHABLE.to_string()
    } else {
        e
    }
}

/// One post as the signed-in player ("signed_out" without the launcher's sign-in).
pub fn post(dir: &std::path::Path, action: &str, args: &Value) -> Result<Value, String> {
    let body = post_for(action, args)?;
    let wait = if action == "check" { CHECK_TIMEOUT } else { TIMEOUT };
    crate::social::call_as_player(dir, &format!("{}/{action}", base()), Some(&body), wait).map_err(friendly)
}

/// The player's own listings and their state.
pub fn mine(dir: &std::path::Path) -> Result<Value, String> {
    crate::social::call_as_player(dir, &format!("{}/mine", base()), None, TIMEOUT).map_err(friendly)
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;
    use tauri::AppHandle;
    use tauri_plugin_opener::OpenerExt;

    /// Check, list, report, edit or remove (see POSTS) as the launcher's signed-in player.
    #[tauri::command]
    pub async fn listing_post(action: String, args: Option<Value>) -> Result<Value, String> {
        let args = args.unwrap_or(Value::Null);
        tauri::async_runtime::spawn_blocking(move || post(&crate::settings::app_dir(), &action, &args)).await.map_err(|e| e.to_string())?
    }

    /// The player's own listings.
    #[tauri::command]
    pub async fn listing_mine() -> Result<Value, String> {
        tauri::async_runtime::spawn_blocking(move || mine(&crate::settings::app_dir())).await.map_err(|e| e.to_string())?
    }

    /// The website's List your mod page in the player's browser (signed out, or to manage listings there).
    #[tauri::command]
    pub fn listing_open(app: AppHandle) -> Result<(), String> {
        app.opener().open_url(PAGE, None::<&str>).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn posts_send_only_their_fields_in_their_shapes() {
        let b = post_for("check", &json!({"url": "https://github.com/alice/Frames", "token": "x"})).unwrap();
        assert_eq!(b, json!({"url": "https://github.com/alice/Frames"}));
        assert!(post_for("check", &json!({"url": "http://github.com/a/b"})).is_err(), "https only");
        assert!(post_for("check", &json!({"url": "https://github.com/a/b c"})).is_err());
        let b = post_for("submit", &json!({"t": "0123456789abcdef0123456789abcdef", "category": "addons", "description": "Frames."})).unwrap();
        assert_eq!(b["category"], "addons");
        assert!(post_for("submit", &json!({"t": "short"})).is_err());
        assert!(post_for("report", &json!({"id": "link:3f9a0c12bd47", "reason": "malware"})).is_ok());
        assert!(post_for("report", &json!({"id": "link:3f9a0c12bd47", "reason": "because"})).is_err(), "reasons from the list");
        assert!(post_for("remove", &json!({"id": "../../admin"})).is_err());
        assert!(post_for("delete-everything", &json!({})).is_err());
        assert!(post_for("edit", &json!({"id": "3f9a0c12bd47", "description": 5})).is_err(), "strings only");
    }

    #[test]
    fn needs_the_launchers_sign_in_and_a_safe_base() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(post(d.path(), "check", &json!({"url": "https://github.com/a/b"})).unwrap_err(), "signed_out");
        assert_eq!(mine(d.path()).unwrap_err(), "signed_out");
        std::env::set_var("LEYLINE_LISTINGS_API", "http://evil.example.com/api");
        assert_eq!(base(), API_BASE);
        std::env::remove_var("LEYLINE_LISTINGS_API");
        assert!(PAGE.starts_with("https://") && API_BASE.starts_with("https://"));
    }
}
