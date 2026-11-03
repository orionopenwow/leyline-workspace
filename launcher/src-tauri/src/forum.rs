//! The Forum page (`ui/forum.js`): reading the website's forum (`stream/forum.py`, its JSON under `/wrath/forum/api`).
//!
//! Reading needs no sign-in: the forum's JSON is public. Posting (a new thread, a reply, a report) goes from here too,
//! with the launcher's own Leyline sign-in (`crate::social`'s token, `Authorization: Bearer`): no second sign-in.
//! The forum checks the same rules as its website forms (`stream/forum.py` api_post). Signed out, New thread and Reply
//! still open the forum's own pages in the player's browser.
//!
//! The UI never names a URL: it names an action from `ACTIONS` with plain parameters, as with `social.rs`. A link
//! inside a post opens only when it came in a reply from the forum and is a plain https address (`seen`).

use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

/// The official forum's JSON.
pub const API_BASE: &str = crate::site!("/wrath/forum/api");
/// The forum's pages on the website (New thread, a thread, Reply open there).
pub const PAGE: &str = crate::site!("/wrath/leyline/forum");
pub const TIMEOUT: Duration = Duration::from_secs(8);
/// What the player reads when the forum can't be reached (never the network library's text).
pub const UNREACHABLE: &str = "The forum isn't reachable right now. Try again, or ask on the Discord.";

/// What the UI may ask: (action, the parameters it may send).
pub const ACTIONS: &[(&str, &[&str])] = &[
    ("categories", &[]),
    ("latest", &["n"]),
    ("threads", &["cat", "page"]),
    ("thread", &["id", "page"]),
    ("search", &["q"]),
];

/// What the UI may post: (action, the parameters it may send). Strings and whole numbers only.
pub const POSTS: &[(&str, &[&str])] = &[
    ("new", &["cat", "title", "body"]),
    ("reply", &["thread", "body"]),
    ("report", &["post", "reason", "note"]),
];
/// The longest text the UI may send (the forum's own limits are lower: 100 for a title, 5000 for a post).
const POST_MAX: usize = 6000;

/// The path and JSON body for a post. Unknown actions are refused, unknown parameters dropped; ids are numbers, a
/// category or reason plain letters, texts strings of at most POST_MAX characters.
pub fn post_for(action: &str, args: &Value) -> Result<(String, Value), String> {
    let (_, keys) = POSTS.iter().find(|a| a.0 == action).ok_or("Unknown action.")?;
    let mut body = serde_json::Map::new();
    for k in keys.iter() {
        let v = match args.get(*k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) if n.is_u64() => n.to_string(),
            None | Some(Value::Null) => continue,
            _ => return Err("Bad parameter.".into()),
        };
        let ok = match *k {
            "thread" | "post" => !v.is_empty() && v.len() <= 9 && v.bytes().all(|b| b.is_ascii_digit()),
            "cat" | "reason" => plain(&v, 16),
            _ => v.chars().count() <= POST_MAX,
        };
        if !ok {
            return Err("Bad parameter.".into());
        }
        let val = if *k == "thread" || *k == "post" { Value::from(v.parse::<u64>().unwrap_or(0)) } else { Value::String(v) };
        body.insert((*k).to_string(), val);
    }
    Ok((action.to_string(), Value::Object(body)))
}

/// One post to the forum as the signed-in player ("signed_out" without the launcher's sign-in).
pub fn post(dir: &std::path::Path, action: &str, args: &Value) -> Result<Value, String> {
    let (path, body) = post_for(action, args)?;
    crate::social::post_as_player(dir, &format!("{}/{path}", base()), &body).map_err(|e| {
        if e.starts_with("Can't reach") {
            UNREACHABLE.to_string()
        } else {
            e
        }
    })
}

/// The API base: the official one, or LEYLINE_FORUM_API for a test server (https, or http on this PC only).
pub fn base() -> String {
    match std::env::var("LEYLINE_FORUM_API") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => {
            b.trim_end_matches('/').to_string()
        }
        _ => API_BASE.to_string(),
    }
}

fn plain(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Percent-encoding for a search text: letters and digits kept, everything else as %XX (UTF-8).
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// The path and query for an action. Unknown actions are refused, unknown parameters dropped; ids and pages are
/// numbers, a category plain letters, a search at most 60 characters with no control characters.
pub fn request_for(action: &str, args: &Value) -> Result<String, String> {
    let (_, keys) = ACTIONS.iter().find(|a| a.0 == action).ok_or("Unknown action.")?;
    let mut q = Vec::new();
    for k in keys.iter() {
        let v = match args.get(*k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => continue,
        };
        let ok = match *k {
            "q" => !v.trim().is_empty() && v.chars().count() <= 60 && !v.chars().any(char::is_control),
            "cat" => plain(&v, 16),
            _ => v.len() <= 9 && !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()),
        };
        if !ok {
            return Err("Bad parameter.".into());
        }
        q.push(format!("{k}={}", if *k == "q" { encode(v.trim()) } else { v }));
    }
    Ok(if q.is_empty() { action.to_string() } else { format!("{action}?{}", q.join("&")) })
}

static SEEN: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Remember the links in a reply's posts (`posts[].parts[].u`), so the UI may open exactly those.
fn remember(v: &Value) {
    let Some(posts) = v.get("posts").and_then(Value::as_array) else { return };
    let mut g = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let set = g.get_or_insert_with(HashSet::new);
    if set.len() > 5000 {
        set.clear();
    }
    for p in posts {
        for part in p.get("parts").and_then(Value::as_array).into_iter().flatten() {
            if let Some(u) = part.get("u").and_then(Value::as_str) {
                if crate::net::plain_https(u) {
                    set.insert(u.to_string());
                }
            }
        }
    }
}

/// True when `url` came in a post the forum sent.
pub fn seen(url: &str) -> bool {
    SEEN.lock().unwrap_or_else(|e| e.into_inner()).as_ref().is_some_and(|s| s.contains(url))
}

/// One read from the forum's JSON.
pub fn call(action: &str, args: &Value) -> Result<Value, String> {
    let path = request_for(action, args)?;
    let body = crate::net::get(&format!("{}/{path}", base()), TIMEOUT)
        .map_err(|e| {
            if e.contains("404") {
                return "That isn't on the forum any more.".to_string();
            }
            eprintln!("forum: {e}"); // the detail for whoever is helping; the player gets a plain line
            UNREACHABLE.to_string()
        })?;
    let v: Value = serde_json::from_str(&body).map_err(|_| "The forum sent something odd. Try again later.".to_string())?;
    remember(&v);
    Ok(v)
}

/// The website page to open for `kind`: "new" (optionally in a category), "thread" or "reply" (a thread id), or
/// "link" (a link from a post, only one the forum sent).
pub fn page_for(kind: &str, arg: &str) -> Result<String, String> {
    match kind {
        "home" => Ok(PAGE.to_string()),
        "new" if arg.is_empty() => Ok(format!("{PAGE}/new")),
        "new" if plain(arg, 16) => Ok(format!("{PAGE}/new?c={arg}")),
        "thread" | "reply" if !arg.is_empty() && arg.len() <= 9 && arg.bytes().all(|b| b.is_ascii_digit()) => {
            Ok(format!("{PAGE}/t/{arg}{}", if kind == "reply" { "/reply" } else { "" }))
        }
        "link" if seen(arg) && crate::net::plain_https(arg) => Ok(arg.to_string()),
        "link" => Err("Leyline only opens links it read on the forum.".into()),
        _ => Err("Unknown page.".into()),
    }
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;
    use tauri::AppHandle;
    use tauri_plugin_opener::OpenerExt;

    /// One read from the forum (see ACTIONS).
    #[tauri::command]
    pub async fn forum_call(action: String, args: Option<Value>) -> Result<Value, String> {
        let args = args.unwrap_or(Value::Null);
        tauri::async_runtime::spawn_blocking(move || call(&action, &args)).await.map_err(|e| e.to_string())?
    }

    /// Post to the forum as the launcher's signed-in player (see POSTS).
    #[tauri::command]
    pub async fn forum_post(action: String, args: Option<Value>) -> Result<Value, String> {
        let args = args.unwrap_or(Value::Null);
        tauri::async_runtime::spawn_blocking(move || post(&crate::settings::app_dir(), &action, &args))
            .await
            .map_err(|e| e.to_string())?
    }

    /// Open a forum page (or a link from a post) in the player's browser.
    #[tauri::command]
    pub fn forum_open(app: AppHandle, kind: String, arg: Option<String>) -> Result<(), String> {
        let url = page_for(&kind, arg.as_deref().unwrap_or(""))?;
        app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_listed_actions_and_plain_parameters() {
        assert_eq!(request_for("categories", &Value::Null).unwrap(), "categories");
        assert_eq!(request_for("threads", &json!({"cat": "help", "page": 2, "x": "y"})).unwrap(), "threads?cat=help&page=2");
        assert_eq!(request_for("thread", &json!({"id": "12"})).unwrap(), "thread?id=12");
        assert_eq!(request_for("search", &json!({"q": " my den & co "})).unwrap(), "search?q=my%20den%20%26%20co");
        assert!(request_for("post", &Value::Null).is_err());
        assert!(request_for("thread", &json!({"id": "1/../x"})).is_err());
        assert!(request_for("threads", &json!({"cat": "a b"})).is_err());
        assert!(request_for("search", &json!({"q": "x".repeat(61)})).is_err());
        assert!(request_for("search", &json!({"q": "a\nb"})).is_err());
    }

    #[test]
    fn pages_are_the_forums_own_and_links_only_ones_it_sent() {
        assert_eq!(page_for("home", "").unwrap(), "https://leylinelauncher.com/wrath/leyline/forum");
        assert_eq!(page_for("new", "dens").unwrap(), "https://leylinelauncher.com/wrath/leyline/forum/new?c=dens");
        assert_eq!(page_for("reply", "7").unwrap(), "https://leylinelauncher.com/wrath/leyline/forum/t/7/reply");
        assert!(page_for("thread", "7x").is_err());
        assert!(page_for("link", "https://evil.example/").is_err());
        remember(&json!({"posts": [{"parts": [{"t": "see "}, {"u": "https://docs.example/a"}, {"u": "javascript:alert(1)"}]}]}));
        assert_eq!(page_for("link", "https://docs.example/a").unwrap(), "https://docs.example/a");
        assert!(page_for("link", "javascript:alert(1)").is_err());
    }

    #[test]
    fn posts_send_only_their_fields() {
        let (p, b) = post_for("new", &json!({"cat": "help", "title": "Stuck", "body": "Hi", "token": "x", "url": "y"})).unwrap();
        assert_eq!((p.as_str(), b), ("new", json!({"cat": "help", "title": "Stuck", "body": "Hi"})));
        let (p, b) = post_for("reply", &json!({"thread": "12", "body": "Thanks"})).unwrap();
        assert_eq!((p.as_str(), b), ("reply", json!({"thread": 12, "body": "Thanks"})));
        let (_, b) = post_for("report", &json!({"post": 7, "reason": "spam"})).unwrap();
        assert_eq!(b, json!({"post": 7, "reason": "spam"}));
        assert!(post_for("delete", &json!({})).is_err());
        assert!(post_for("reply", &json!({"thread": "1/../x", "body": "a"})).is_err());
        assert!(post_for("reply", &json!({"thread": -1, "body": "a"})).is_err());
        assert!(post_for("new", &json!({"cat": "a b", "title": "t", "body": "b"})).is_err());
        assert!(post_for("new", &json!({"cat": "help", "title": ["x"], "body": "b"})).is_err());
        assert!(post_for("reply", &json!({"thread": 1, "body": "x".repeat(POST_MAX + 1)})).is_err());
    }

    #[test]
    fn posting_needs_the_launchers_sign_in() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(post(d.path(), "reply", &json!({"thread": 1, "body": "hi"})).unwrap_err(), "signed_out");
    }

    #[test]
    fn the_test_server_must_be_https_or_this_pc() {
        std::env::set_var("LEYLINE_FORUM_API", "http://evil.example/api");
        assert_eq!(base(), API_BASE);
        std::env::set_var("LEYLINE_FORUM_API", "http://127.0.0.1:9/wrath/forum/api/");
        assert_eq!(base(), "http://127.0.0.1:9/wrath/forum/api");
        std::env::remove_var("LEYLINE_FORUM_API");
    }
}
