//! The Leyline sign-in in the launcher (Home's profile, `ui/social.js`): the name it shows, mod ratings and Hero Key
//! sign-in to community realms use it. The friends and messages actions below are no longer used by the UI (the
//! friends area went on 2026-10-30, in favour of the Discord and the forum); the server still answers them.
//!
//! The server is the Wrath portal's `/wrath/api/social` (`stream/social.py`); docs/SOCIAL.md has the whole picture.
//! Sign-in is the website's own, in the player's browser: the launcher asks for a device code, opens
//! `https://leylinelauncher.com/wrath/link?code=XXXX-XXXX`, the player approves it there (signed in with Google or
//! their Hero Key), and the launcher's next poll gets a session token. Only that token is kept, in
//! `<app data>/social-session.json` (0600 on Mac/Linux): never a password. It is revocable on the link page, and
//! "Sign out" here revokes it on the server too.
//!
//! The UI never names a URL: it names an action from `ACTIONS`, and only the parameters listed for that action are
//! sent. Each call gives up after 8 seconds, except the long poll for live updates (35 s).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
#[cfg(feature = "app")]
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The official server's social API.
pub const API_BASE: &str = crate::site!("/wrath/api/social");
/// Where the session token lives, in the app data folder.
pub const SESSION_FILE: &str = "social-session.json";
pub const TIMEOUT: Duration = Duration::from_secs(8);
pub const LONG_POLL: Duration = Duration::from_secs(35);

/// What the UI may call: (action, method, path, the parameters it may send).
pub const ACTIONS: &[(&str, &str, &str, &[&str])] = &[
    ("me", "GET", "me", &[]),
    ("state", "GET", "state", &[]),
    ("events", "GET", "events", &["since"]),
    ("thread", "GET", "thread", &["with", "before"]),
    // the profile (Manage account, ui/account.js): name, picture, bio and the first-time step (stream/profiles.py)
    ("profile", "POST", "profile", &["name", "main", "show_main", "offline", "bio", "avatar", "setup"]),
    ("request", "POST", "request", &["to"]),
    ("accept", "POST", "accept", &["pid"]),
    ("decline", "POST", "decline", &["pid"]),
    ("remove", "POST", "remove", &["pid"]),
    ("block", "POST", "block", &["pid", "to"]),
    ("unblock", "POST", "unblock", &["pid"]),
    ("send", "POST", "send", &["to", "text"]),
    ("read", "POST", "read", &["with"]),
    ("report", "POST", "report", &["pid", "reason", "note"]),
    // Hero Key sign-in to a community realm (crate::herokey, docs/HERO-KEY-SSO.md)
    ("herokey", "POST", "herokey", &["realm"]),
    // Your Hero Key (stream/herokeys.py, Manage account): what the key is up to, a new key, saved, an optional password
    ("herokey_status", "GET", "herokey", &[]),
    ("herokey_new", "POST", "herokey/new", &[]),
    ("herokey_saved", "POST", "herokey/saved", &[]),
    ("password", "POST", "password", &["password", "remove"]),
];

/// A Hero Key as the website makes them: `ovk1.`, a 10-character id, a dot and 43 base64url characters.
pub fn key_ok(k: &str) -> bool {
    let Some(rest) = k.strip_prefix("ovk1.") else { return false };
    let Some((id, sec)) = rest.split_once('.') else { return false };
    id.len() == 10
        && id.bytes().all(|b| b.is_ascii_alphanumeric())
        && sec.len() == 43
        && sec.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The key in whatever the player pasted (spaces and line breaks taken out), or None.
pub fn find_key(pasted: &str) -> Option<String> {
    let joined: String = pasted.chars().filter(|c| !c.is_whitespace()).take(400).collect();
    let at = joined.find("ovk1.")?;
    let k: String = joined[at..].chars().take(5 + 10 + 1 + 43).collect();
    key_ok(&k).then_some(k)
}

/// A call that signs in (no token yet): the answer's token is kept, the rest (me, and a new account's key) goes back.
fn sign_in_call(dir: &Path, path: &str, body: &Value) -> Result<Value, String> {
    let r = crate::addresses::try_both(&format!("{}/{path}", base()), |u| post_json(agent(TIMEOUT).post(u), body)).map_err(|e| match e {
        // a refused key or password is the server's own words, not "signed_out"
        ureq::Error::Status(401, resp) => resp
            .into_string()
            .ok()
            .and_then(|b| serde_json::from_str::<Value>(&b).ok())
            .and_then(|v| v.get("error").and_then(Value::as_str).map(|s| s.chars().take(200).collect()))
            .unwrap_or_else(|| "That didn't work.".into()),
        e => failure(e),
    })?;
    let mut v = read_json(r)?;
    save_token(dir, v.get("token").and_then(Value::as_str).unwrap_or(""))?;
    if let Some(o) = v.as_object_mut() {
        o.remove("token");
    }
    Ok(v)
}

/// Sign in with a pasted Hero Key (the key goes to the website once, over https, and is not kept here).
pub fn sign_in_key(dir: &Path, pasted: &str) -> Result<Value, String> {
    let key = find_key(pasted).ok_or("That doesn't look like a Hero Key. It starts with ovk1. Paste the whole line.")?;
    sign_in_call(dir, "herokey/signin", &serde_json::json!({ "key": key }))
}

/// A new account (a made-up name, its Hero Key): {key, me}. The key is shown to the player once and not kept here.
pub fn new_account(dir: &Path) -> Result<Value, String> {
    sign_in_call(dir, "account/new", &serde_json::json!({}))
}

/// Sign in with Name#1234 and the password the player set.
pub fn sign_in_password(dir: &Path, name: &str, password: &str) -> Result<Value, String> {
    if name.len() > 40 || password.len() > 128 || password.is_empty() {
        return Err("Type your name with its #tag and your password.".into());
    }
    sign_in_call(dir, "password/signin", &serde_json::json!({ "name": name.trim(), "password": password }))
}

/// The file name the save dialog offers: ours, or a plain one ending in .txt.
pub fn text_file_name(name: &str) -> String {
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).take(60).collect();
    let clean = clean.trim().trim_start_matches('.').to_string();
    let stem = clean.strip_suffix(".txt").unwrap_or(&clean).trim();
    if stem.is_empty() { "Leyline Hero Key.txt".into() } else { format!("{stem}.txt") }
}

/// The API base: the official one, or LEYLINE_SOCIAL_API for a test server (https, or http on this PC only).
pub fn base() -> String {
    match std::env::var("LEYLINE_SOCIAL_API") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => {
            b.trim_end_matches('/').to_string()
        }
        _ => API_BASE.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Saved {
    token: String,
    base: String,
}

fn session_path(dir: &Path) -> PathBuf {
    dir.join(SESSION_FILE)
}

fn token_ok(t: &str) -> bool {
    t.len() == 43 && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The saved token for this API base, if any.
pub fn load_token(dir: &Path) -> Option<String> {
    let s: Saved = serde_json::from_slice(&std::fs::read(session_path(dir)).ok()?).ok()?;
    (s.base == base() && token_ok(&s.token)).then_some(s.token)
}

/// Keep the token (atomically; readable by this user only on Mac/Linux).
pub fn save_token(dir: &Path, token: &str) -> Result<(), String> {
    if !token_ok(token) {
        return Err("The server sent a bad sign-in.".into());
    }
    let bytes = serde_json::to_vec(&Saved { token: token.into(), base: base() }).map_err(|e| e.to_string())?;
    let path = session_path(dir);
    crate::settings::write_atomic(&path, &bytes).map_err(|e| format!("Could not save the sign-in: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn forget(dir: &Path) {
    let _ = std::fs::remove_file(session_path(dir));
}

fn agent(read: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(TIMEOUT)
        .timeout_read(read)
        .timeout_write(TIMEOUT)
        .user_agent(concat!("Leyline/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// POST a JSON body (ureq here is built without its json feature).
fn post_json(req: ureq::Request, body: &Value) -> Result<ureq::Response, ureq::Error> {
    req.set("Content-Type", "application/json").send_string(&body.to_string())
}

fn read_json(r: ureq::Response) -> Result<Value, String> {
    let body = r.into_string().map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|_| "The server's answer didn't look right.".to_string())
}

/// Why a call failed: "signed_out" (the token is gone or revoked) or the server's own words.
fn failure(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(401, _) => "signed_out".into(),
        ureq::Error::Status(code, resp) => resp
            .into_string()
            .ok()
            .and_then(|b| serde_json::from_str::<Value>(&b).ok())
            .and_then(|v| v.get("error").and_then(Value::as_str).map(|s| s.chars().take(200).collect()))
            .unwrap_or_else(|| format!("The server answered {code}.")),
        ureq::Error::Transport(_) => "Can't reach the Leyline server. Check your connection.".into(),
    }
}

/// A device sign-in waiting for the player's approval in the browser.
#[derive(Debug, Clone)]
pub struct Pending {
    pub device_code: String,
    pub user_code: String,
    pub verify_url: String,
    pub interval: u64,
    pub until: Instant,
    /// What the browser opens: verify_url, plus `&via=<way>` when the player picked a way (`open_url`).
    pub open: String,
}

/// The sign-in ways the launcher offers (the website's own: stream/profiles.py WAYS).
pub const WAYS: &[&str] = &["herokey", "google", "discord"];

/// The approval page with the way the player picked (the page goes straight to it); an unknown way is left off.
pub fn open_url(verify_url: &str, via: Option<&str>) -> String {
    match via {
        Some(v) if WAYS.contains(&v) => format!("{verify_url}&via={v}"),
        _ => verify_url.to_string(),
    }
}

/// Which ways the website offers right now ({herokey, google, discord}: booleans only), no sign-in needed.
pub fn ways() -> Result<Value, String> {
    let v = read_json(crate::addresses::try_both(&format!("{}/ways", base()), |u| agent(TIMEOUT).get(u).call()).map_err(failure)?)?;
    Ok(sanitize_ways(&v))
}

fn sanitize_ways(v: &Value) -> Value {
    let mut out = Map::new();
    for w in WAYS {
        out.insert((*w).to_string(), Value::Bool(*w == "herokey" || v.get(*w).and_then(Value::as_bool).unwrap_or(false)));
    }
    Value::Object(out)
}

/// What the UI sees of a sign-in in progress.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SignIn {
    /// "pending", "signed_in", "denied", "expired", "signed_out"
    pub state: String,
    pub user_code: String,
    pub verify_url: String,
}

/// The approval page address the server gave, if it is the one we expect (our server's /wrath/link with a code).
pub fn verify_url_ok(url: &str, user_code: &str) -> bool {
    let b = base();
    let site = b.trim_end_matches("/api/social");
    let want = format!("{site}/link?code={user_code}");
    // on either of the site's names: a sign-in started on the old one (addresses.rs fallback) is shown there
    (url == want || crate::addresses::fallback(&want).as_deref() == Some(url)) && code_ok(user_code)
}

fn code_ok(c: &str) -> bool {
    let ok = |s: &str| s.len() == 4 && s.bytes().all(|b| b"BCDFGHJKLMNPQRSTVWXZ".contains(&b));
    matches!(c.split_once('-'), Some((a, b)) if ok(a) && ok(b))
}

/// Ask the server for a device code.
pub fn start() -> Result<Pending, String> {
    let v = read_json(crate::addresses::try_both(&format!("{}/device/start", base()), |u| post_json(agent(TIMEOUT).post(u), &serde_json::json!({}))).map_err(failure)?)?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let p = Pending {
        device_code: s("device_code"),
        user_code: s("user_code"),
        verify_url: s("verify_url"),
        interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5).clamp(2, 30),
        until: Instant::now() + Duration::from_secs(v.get("expires_in").and_then(Value::as_u64).unwrap_or(600).min(1800)),
        open: String::new(),
    };
    if !token_ok(&p.device_code) || !verify_url_ok(&p.verify_url, &p.user_code) {
        return Err("The server's answer didn't look right.".into());
    }
    Ok(p)
}

/// One poll: "pending", "slow_down", "denied", "expired", or "signed_in" (the token is saved).
pub fn poll(dir: &Path, p: &Pending) -> Result<String, String> {
    if Instant::now() > p.until {
        return Ok("expired".into());
    }
    let body = serde_json::json!({ "device_code": p.device_code });
    let v = read_json(crate::addresses::try_both(&format!("{}/device/poll", base()), |u| post_json(agent(TIMEOUT).post(u), &body)).map_err(failure)?)?;
    match v.get("status").and_then(Value::as_str).unwrap_or("") {
        "ok" => {
            save_token(dir, v.get("token").and_then(Value::as_str).unwrap_or(""))?;
            Ok("signed_in".into())
        }
        s @ ("pending" | "slow_down" | "denied" | "expired") => Ok(s.into()),
        _ => Err("The server's answer didn't look right.".into()),
    }
}

/// The request for an action: (method, path with query, JSON body for a POST). Unknown actions and parameters are
/// refused or dropped; GET parameters must be plain (letters, digits, - and _).
pub fn request_for(action: &str, args: &Value) -> Result<(&'static str, String, Option<Value>), String> {
    let (_, method, path, keys) = ACTIONS.iter().find(|a| a.0 == action).ok_or("Unknown action.")?;
    let empty = Map::new();
    let obj = args.as_object().unwrap_or(&empty);
    if *method == "GET" {
        let mut q = Vec::new();
        for k in keys.iter() {
            if let Some(v) = obj.get(*k) {
                let s = match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                if s.is_empty() || s.len() > 40 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                    return Err("Bad parameter.".into());
                }
                q.push(format!("{k}={s}"));
            }
        }
        let p = if q.is_empty() { path.to_string() } else { format!("{path}?{}", q.join("&")) };
        return Ok(("GET", p, None));
    }
    let body: Map<String, Value> = keys.iter().filter_map(|k| obj.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
    Ok(("POST", path.to_string(), Some(Value::Object(body))))
}

/// Call the API as the signed-in player. A 401 forgets the token and answers "signed_out".
pub fn call(dir: &Path, action: &str, args: &Value) -> Result<Value, String> {
    let token = load_token(dir).ok_or("signed_out")?;
    let (method, path, body) = request_for(action, args)?;
    let a = agent(if action == "events" { LONG_POLL } else { TIMEOUT });
    let url = format!("{}/{path}", base());
    let auth = format!("Bearer {token}");
    let body = body.unwrap_or(Value::Null);
    let res = crate::addresses::try_both(&url, |u| {
        if method == "GET" {
            a.get(u).set("Authorization", &auth).call()
        } else {
            post_json(a.post(u).set("Authorization", &auth), &body)
        }
    });
    match res {
        Ok(r) => read_json(r),
        Err(e) => {
            let msg = failure(e);
            if msg == "signed_out" {
                forget(dir);
            }
            Err(msg)
        }
    }
}

/// POST `body` to another of the website's APIs (the forum's, `crate::forum`) as the signed-in player, with the same
/// token and the same rules as `call`: "signed_out" without a token or on a 401 (the token is then forgotten), the
/// server's own words on a refusal. `url` is built by the caller from its fixed base, never from the UI.
pub fn post_as_player(dir: &Path, url: &str, body: &Value) -> Result<Value, String> {
    let token = load_token(dir).ok_or("signed_out")?;
    let auth = format!("Bearer {token}");
    match crate::addresses::try_both(url, |u| post_json(agent(TIMEOUT).post(u).set("Authorization", &auth), body)) {
        Ok(r) => read_json(r),
        Err(e) => {
            let msg = failure(e);
            if msg == "signed_out" {
                forget(dir);
            }
            Err(msg)
        }
    }
}

/// A GET (body None) or POST as the signed-in player with its own read timeout ("signed_out" without the sign-in).
/// For calls the server needs longer for, such as List your mod's check (it fetches the mod once: listing.rs).
pub fn call_as_player(dir: &Path, url: &str, body: Option<&Value>, read: Duration) -> Result<Value, String> {
    let token = load_token(dir).ok_or("signed_out")?;
    let auth = format!("Bearer {token}");
    let r = crate::addresses::try_both(url, |u| match body {
        Some(b) => post_json(agent(read).post(u).set("Authorization", &auth), b),
        None => agent(read).get(u).set("Authorization", &auth).call(),
    });
    match r {
        Ok(r) => read_json(r),
        Err(e) => {
            let msg = failure(e);
            if msg == "signed_out" {
                forget(dir);
            }
            Err(msg)
        }
    }
}

/// Sign out: revoke the token on the server (best effort) and forget it here.
pub fn sign_out(dir: &Path) {
    if let Some(token) = load_token(dir) {
        let auth = format!("Bearer {token}");
        let _ = crate::addresses::try_both(&format!("{}/token/revoke", base()), |u| post_json(agent(TIMEOUT).post(u).set("Authorization", &auth), &serde_json::json!({})));
    }
    forget(dir);
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;

    static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
    use tauri::AppHandle;
    use tauri_plugin_opener::OpenerExt;

    fn dir() -> PathBuf {
        crate::settings::app_dir()
    }

    async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
        tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
    }

    /// Signed in or not (no network).
    #[tauri::command]
    pub fn social_status() -> bool {
        load_token(&dir()).is_some()
    }

    /// Start a sign-in: get a code and open the approval page in the player's browser (with the way they picked:
    /// herokey, google or discord, which the page goes straight to).
    #[tauri::command]
    pub async fn social_signin_start(app: AppHandle, via: Option<String>) -> Result<SignIn, String> {
        let mut p = blocking(start).await?;
        p.open = open_url(&p.verify_url, via.as_deref());
        app.opener().open_url(p.open.clone(), None::<&str>).map_err(|e| e.to_string())?;
        let out = SignIn { state: "pending".into(), user_code: p.user_code.clone(), verify_url: p.verify_url.clone() };
        *PENDING.lock().map_err(|_| "lock")? = Some(p);
        Ok(out)
    }

    /// Open the approval page again (the same code).
    #[tauri::command]
    pub fn social_signin_reopen(app: AppHandle) -> Result<(), String> {
        let url = PENDING.lock().map_err(|_| "lock")?.as_ref().map(|p| p.open.clone()).ok_or("No sign-in in progress.")?;
        app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
    }

    /// Poll the sign-in in progress (the UI calls this every few seconds while the page waits).
    #[tauri::command]
    pub async fn social_signin_poll() -> Result<SignIn, String> {
        let p = PENDING.lock().map_err(|_| "lock")?.clone();
        let Some(p) = p else { return Ok(SignIn { state: "signed_out".into(), user_code: String::new(), verify_url: String::new() }) };
        let pc = p.clone();
        let state = blocking(move || poll(&dir(), &pc)).await?;
        if state != "pending" && state != "slow_down" {
            *PENDING.lock().map_err(|_| "lock")? = None;
        }
        let state = if state == "slow_down" { "pending".to_string() } else { state };
        Ok(SignIn { state, user_code: p.user_code, verify_url: p.verify_url })
    }

    /// Cancel a sign-in in progress.
    #[tauri::command]
    pub fn social_signin_cancel() {
        if let Ok(mut p) = PENDING.lock() {
            *p = None;
        }
    }

    /// One API call by action name (see ACTIONS).
    #[tauri::command]
    pub async fn social_call(action: String, args: Option<Value>) -> Result<Value, String> {
        blocking(move || call(&dir(), &action, &args.unwrap_or(Value::Null))).await
    }

    /// The sign-in ways the website offers (Manage account's buttons): no sign-in needed.
    #[tauri::command]
    pub async fn social_ways() -> Result<Value, String> {
        blocking(ways).await
    }

    /// Sign in with a pasted Hero Key (Manage account, the account panel).
    #[tauri::command]
    pub async fn social_key_signin(key: String) -> Result<Value, String> {
        blocking(move || sign_in_key(&dir(), &key)).await
    }

    /// Get your Hero Key: a new account; the answer carries the key to show once.
    #[tauri::command]
    pub async fn social_new_account() -> Result<Value, String> {
        blocking(|| new_account(&dir())).await
    }

    /// Sign in with Name#1234 and a password.
    #[tauri::command]
    pub async fn social_password_signin(name: String, password: String) -> Result<Value, String> {
        blocking(move || sign_in_password(&dir(), &name, &password)).await
    }

    /// Save as text file: the native save dialog, then the text (a Hero Key's .txt; at most 4 KB). false: cancelled.
    #[tauri::command]
    pub async fn save_text_file(app: AppHandle, name: String, text: String) -> Result<bool, String> {
        use tauri_plugin_dialog::DialogExt;
        if text.len() > 4096 {
            return Err("That's too long for a key file.".into());
        }
        let file = text_file_name(&name);
        let picked = tauri::async_runtime::spawn_blocking(move || {
            app.dialog().file().set_title("Save your Hero Key").set_file_name(&file).add_filter("Text file", &["txt"]).blocking_save_file()
        })
        .await
        .map_err(|e| e.to_string())?;
        let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(false) };
        let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) { path } else { path.with_extension("txt") };
        std::fs::write(&path, text.as_bytes()).map_err(|e| format!("Couldn't save the file: {e}"))?;
        Ok(true)
    }

    #[tauri::command]
    pub async fn social_signout() -> Result<(), String> {
        blocking(|| {
            sign_out(&dir());
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn token_saved_read_and_forgotten() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load_token(d.path()), None);
        assert!(save_token(d.path(), "short").is_err());
        let t = "a".repeat(43);
        save_token(d.path(), &t).unwrap();
        assert_eq!(load_token(d.path()), Some(t.clone()));
        let raw = std::fs::read_to_string(d.path().join(SESSION_FILE)).unwrap();
        assert!(raw.contains(API_BASE) && !raw.to_lowercase().contains("password"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.path().join(SESSION_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "only this user may read the session");
        }
        forget(d.path());
        assert_eq!(load_token(d.path()), None);
    }

    #[test]
    fn a_token_for_another_server_is_not_used() {
        let d = tempfile::tempdir().unwrap();
        let s = Saved { token: "b".repeat(43), base: "https://elsewhere.example/api".into() };
        std::fs::write(d.path().join(SESSION_FILE), serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(load_token(d.path()), None);
    }

    #[test]
    fn only_listed_actions_and_parameters() {
        assert!(request_for("https://evil.example", &json!({})).is_err());
        assert!(request_for("token/revoke", &json!({})).is_err());
        let (m, p, b) = request_for("send", &json!({"to": "abcdefghij", "text": "hi", "url": "x", "token": "y"})).unwrap();
        assert_eq!((m, p.as_str()), ("POST", "send"));
        assert_eq!(b.unwrap(), json!({"to": "abcdefghij", "text": "hi"}));
        let (m, p, b) = request_for("thread", &json!({"with": "abcdefghij", "before": 42, "x": 1})).unwrap();
        assert_eq!((m, p.as_str(), b), ("GET", "thread?with=abcdefghij&before=42", None));
        assert!(request_for("events", &json!({"since": "1&x=2"})).is_err());
        assert_eq!(request_for("state", &Value::Null).unwrap().1, "state");
    }

    #[test]
    fn the_approval_page_must_be_ours() {
        assert!(verify_url_ok("https://leylinelauncher.com/wrath/link?code=BCDF-GHJK", "BCDF-GHJK"));
        assert!(verify_url_ok("https://play.openvanilla.app/wrath/link?code=BCDF-GHJK", "BCDF-GHJK"));   // the old name
        assert!(!verify_url_ok("https://evil.example/wrath/link?code=BCDF-GHJK", "BCDF-GHJK"));
        assert!(!verify_url_ok("https://play.openvanilla.app/wrath/link?code=BCDF-GHJK&x=1", "BCDF-GHJK"));
        assert!(!verify_url_ok("https://play.openvanilla.app/wrath/link?code=ABCD-EFGH", "ABCD-EFGH"));
    }

    #[test]
    fn profile_sends_only_its_fields() {
        let (m, p, b) = request_for("profile", &json!({"bio": "hi", "avatar": "moon", "setup": true, "name": "A1", "token": "x", "url": "y"})).unwrap();
        assert_eq!((m, p.as_str()), ("POST", "profile"));
        assert_eq!(b.unwrap(), json!({"bio": "hi", "avatar": "moon", "setup": true, "name": "A1"}));
    }

    #[test]
    fn the_way_rides_on_the_approval_page_only_when_known() {
        let u = "https://play.openvanilla.app/wrath/link?code=BCDF-GHJK";
        assert_eq!(open_url(u, Some("google")), format!("{u}&via=google"));
        assert_eq!(open_url(u, Some("discord")), format!("{u}&via=discord"));
        assert_eq!(open_url(u, Some("herokey")), format!("{u}&via=herokey"));
        assert_eq!(open_url(u, Some("https://evil.example")), u);
        assert_eq!(open_url(u, Some("google&x=1")), u);
        assert_eq!(open_url(u, None), u);
    }

    #[test]
    fn ways_are_booleans_and_the_hero_key_is_always_there() {
        assert_eq!(sanitize_ways(&json!({"google": true, "discord": "yes", "evil": true})), json!({"herokey": true, "google": true, "discord": false}));
        assert_eq!(sanitize_ways(&Value::Null), json!({"herokey": true, "google": false, "discord": false}));
    }

    #[test]
    fn hero_keys_are_checked_before_they_go_anywhere() {
        let k = format!("ovk1.AbCdEf1234.{}", "a_b-".repeat(10) + "xyz");
        assert!(key_ok(&k));
        assert_eq!(find_key(&format!("  Key: {}\n{}  ", &k[..30], &k[30..])), Some(k.clone()));
        assert!(!key_ok("ovk1.short.x") && !key_ok(&k.replace("ovk1", "ovk2")) && !key_ok(&format!("{k}!")));
        assert_eq!(find_key("hello"), None);
        let d = tempfile::tempdir().unwrap();
        assert!(sign_in_key(d.path(), "not a key").unwrap_err().contains("ovk1."));
        assert!(sign_in_password(d.path(), "Alice#1234", "").is_err());
        assert_eq!(load_token(d.path()), None);
    }

    #[test]
    fn hero_key_actions_send_only_their_fields() {
        assert_eq!(request_for("herokey_status", &json!({"x": 1})).unwrap(), ("GET", "herokey".to_string(), None));
        let (m, p, b) = request_for("herokey_new", &json!({"key": "ovk1.x"})).unwrap();
        assert_eq!((m, p.as_str(), b.unwrap()), ("POST", "herokey/new", json!({})));
        let (_, p, b) = request_for("password", &json!({"password": "correct horse", "token": "t"})).unwrap();
        assert_eq!((p.as_str(), b.unwrap()), ("password", json!({"password": "correct horse"})));
        assert!(request_for("account/new", &json!({})).is_err(), "signing in has its own commands");
    }

    #[test]
    fn the_text_file_is_always_a_plain_txt() {
        assert_eq!(text_file_name("Leyline Hero Key.txt"), "Leyline Hero Key.txt");
        assert_eq!(text_file_name("../../etc/passwd"), "etcpasswd.txt");
        assert_eq!(text_file_name(""), "Leyline Hero Key.txt");
        assert_eq!(text_file_name(".bashrc"), "bashrc.txt");
        assert_eq!(text_file_name("C:\\Windows\\evil.exe"), "CWindowsevil.exe.txt");
    }

    #[test]
    fn signed_out_without_a_token() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(call(d.path(), "state", &Value::Null).unwrap_err(), "signed_out");
    }
}
