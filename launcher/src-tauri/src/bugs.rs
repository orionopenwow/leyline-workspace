//! Help › Report a bug (`ui/bugreport.js`): one report to the website's bug list (`stream/bugreport.py`,
//! `POST /wrath/bugs/api/report`), sent as the launcher's signed-in player (`crate::social`'s token,
//! `Authorization: Bearer`), so it joins the same backlog as the website's form, `.bug` in the game and the
//! Discord's #bug-reports (docs/BUGS-MANIFEST.md).
//!
//! What goes: the player's words, where (a kind from the site's own list and a short page/realm line), the launcher's
//! version and, only when the player ticks it, the last lines of the launcher's own log (the UI's recent warnings and
//! errors, `window.LeylineBugReport`). Everything is cleaned here before it leaves: home-folder user names in paths,
//! anything token-like (Bearer, `token=`, long keys, the player's own sign-in token), emails and IP addresses. The
//! site redacts again as it arrives. Within the site's limits: 10 to 1500 characters of text, a page line of 120, an
//! app line of 40, a body under 8 KB.

use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

/// The official endpoint.
pub const API: &str = "https://play.openvanilla.app/wrath/bugs/api/report";
pub const TIMEOUT: Duration = Duration::from_secs(8);
/// The site's limits (stream/bugreport.py MIN_TEXT, TEXT_MAX, PAGE_MAX, APP_MAX, FORM_MAX).
pub const MIN_TEXT: usize = 10;
pub const TEXT_MAX: usize = 1500;
pub const PAGE_MAX: usize = 120;
pub const APP_MAX: usize = 40;
pub const BODY_MAX: usize = 8192;
/// The most log lines the UI may hand over, and the longest line kept.
pub const LOG_LINES: usize = 40;
const LOG_LINE_MAX: usize = 300;
/// The site's "where" kinds (stream/bugreport.py WHERE).
pub const WHERE: &[&str] = &["site", "launcher", "game", "den", "host", "mods", "other"];
/// What heads the log tail inside the text.
pub const LOG_HEAD: &str = "\n\n--- launcher log (last lines) ---\n";

/// The endpoint: the official one, or LEYLINE_BUGS_API for a test server (https, or http on this PC only).
pub fn api() -> String {
    match std::env::var("LEYLINE_BUGS_API") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => b,
        _ => API.to_string(),
    }
}

// ------------------------------------------------------------------------------------------------- redaction

fn is_tok(c: char) -> bool {
    c.is_ascii_alphanumeric() || "._~+/=-".contains(c)
}

/// Words that put a secret after them ("token: x", "password=x", "?t=x").
const SECRET_KEYS: &[&str] = &[
    "pass", "password", "passwd", "pwd", "pw", "token", "secret", "apikey", "api_key", "api-key", "key", "auth",
    "session", "cookie", "code", "sig", "t", "access_token",
];

/// Replace `/home/<name>`, `/Users/<name>` and `C:\Users\<name>` (any drive, `Documents and Settings` too) with
/// `[user]`.
fn redact_paths(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while !rest.is_empty() {
        for pre in ["/home/", "/Users/"] {
            if rest.starts_with(pre) {
                out.push_str(pre);
                rest = &rest[pre.len()..];
                let end = rest.find(|c: char| c == '/' || c.is_whitespace()).unwrap_or(rest.len());
                if end > 0 {
                    out.push_str("[user]");
                }
                rest = &rest[end..];
                continue 'outer;
            }
        }
        let b = rest.as_bytes();
        if b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/') {
            let after = rest[3..].trim_start_matches(['\\', '/']);
            let skip = rest.len() - after.len();
            for dir in ["Users", "Documents and Settings"] {
                if after.len() > dir.len() && after[..dir.len()].eq_ignore_ascii_case(dir) && matches!(after.as_bytes()[dir.len()], b'\\' | b'/') {
                    let tail = after[dir.len()..].trim_start_matches(['\\', '/']);
                    let head = rest.len() - tail.len();
                    out.push_str(&rest[..head]);
                    let end = tail.find(|c: char| c == '\\' || c == '/' || c.is_whitespace()).unwrap_or(tail.len());
                    if end > 0 {
                        out.push_str("[user]");
                    }
                    rest = &tail[end..];
                    continue 'outer;
                }
            }
            out.push_str(&rest[..skip.min(3)]);
            rest = &rest[skip.min(3)..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn looks_ipv4(w: &str) -> bool {
    let parts: Vec<&str> = w.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u16>().is_ok_and(|n| n <= 255))
}

fn looks_ipv6(w: &str) -> bool {
    w.matches(':').count() >= 3 && w.chars().all(|c| c.is_ascii_hexdigit() || c == ':') && w.chars().any(|c| c.is_ascii_hexdigit())
}

fn looks_email(w: &str) -> bool {
    let Some((a, d)) = w.split_once('@') else { return false };
    !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')
}

/// A long run of letters and digits with both in it (a key, a token, a hash), or a JWT-like a.b.c.
fn looks_secret(w: &str) -> bool {
    let core: String = w.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    let mixed = core.chars().any(|c| c.is_ascii_digit()) && core.chars().any(|c| c.is_ascii_alphabetic());
    (mixed && core.len() >= 24 && !w.contains('/')) || (w.split('.').count() == 3 && w.split('.').all(|p| p.len() >= 6) && w.len() >= 50)
}

/// One whitespace-free word, cleaned.
fn redact_word(w: &str, prev: &str) -> String {
    let pl = prev.trim_end_matches([':', '=']).to_ascii_lowercase();
    if ["bearer", "bot", "basic"].contains(&pl.as_str()) && w.len() >= 12 {
        return "[redacted]".into();
    }
    if (SECRET_KEYS.contains(&pl.as_str()) && (prev.ends_with(':') || prev.ends_with('='))) && !w.is_empty() {
        return "[redacted]".into();
    }
    // key=value and key:value inside the word (also in a query string: ?t=..&token=..)
    let mut out = String::new();
    let mut rest = w;
    while let Some(i) = rest.find(['=', ':']) {
        let (k, v) = (&rest[..i], &rest[i + 1..]);
        let key = k.rsplit(['?', '&', '(', '"', '[', '{', ',', ';']).next().unwrap_or(k).to_ascii_lowercase();
        if SECRET_KEYS.contains(&key.as_str()) && !v.is_empty() && !v.starts_with("//") {
            out.push_str(&rest[..=i]);
            let end = v.find(['&', '#', '"', '\'', ',', ';', ')', ']', '}']).unwrap_or(v.len());
            out.push_str("[redacted]");
            rest = &v[end..];
        } else {
            out.push_str(&rest[..=i]);
            rest = v;
        }
    }
    out.push_str(rest);
    // what is left: split into token-ish runs and check each
    let mut res = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, res: &mut String| {
        if !run.is_empty() {
            let r = run.trim_matches('.');
            let lead = &run[..run.len() - run.trim_start_matches('.').len()];
            let trail = &run[lead.len() + r.len()..];
            res.push_str(lead);
            if looks_email(r) {
                res.push_str("[email]");
            } else if looks_ipv4(r) || looks_ipv6(r) {
                res.push_str("[ip]");
            } else if looks_secret(r) {
                res.push_str("[redacted]");
            } else {
                res.push_str(r);
            }
            res.push_str(trail);
            run.clear();
        }
    };
    for c in out.chars() {
        if is_tok(c) || c == '@' || (c == ':' && run.chars().all(|x| x.is_ascii_hexdigit() || x == ':') && !run.is_empty()) {
            run.push(c);
        } else {
            flush(&mut run, &mut res);
            res.push(c);
        }
    }
    flush(&mut run, &mut res);
    res
}

/// Clean one text: control characters out (new lines kept), user names in paths, tokens, keys, emails, IPs, and
/// `own` (the player's sign-in token, when there is one) wherever it appears.
pub fn redact(s: &str, own: Option<&str>) -> String {
    let mut s: String = s.chars().map(|c| if c == '\n' || !(c.is_control() || ('\u{200b}'..='\u{200f}').contains(&c) || ('\u{202a}'..='\u{202e}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c)) { c } else { ' ' }).collect();
    if let Some(t) = own.filter(|t| t.len() >= 8) {
        s = s.replace(t, "[redacted]");
    }
    let s = redact_paths(&s);
    let mut out = String::with_capacity(s.len());
    let mut prev = String::new();
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut first = true;
        for w in line.split(' ') {
            if !first {
                out.push(' ');
            }
            first = false;
            if w.is_empty() {
                continue;
            }
            out.push_str(&redact_word(w, &prev));
            prev = w.to_string();
        }
    }
    out
}

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ------------------------------------------------------------------------------------------------- the request

/// The JSON body for a report, or the player's reason it can't go. `args`: {text, where?, page?, log?: [lines]}.
pub fn request_for(args: &Value, version: &str, own_token: Option<&str>) -> Result<Value, String> {
    let s = |k: &str| match args.get(k) {
        Some(Value::String(v)) => Ok(v.clone()),
        None | Some(Value::Null) => Ok(String::new()),
        _ => Err("Bad parameter.".to_string()),
    };
    let text = redact(s("text")?.trim(), own_token);
    let text = text.trim();
    if text.chars().filter(|c| c.is_alphanumeric()).count() < MIN_TEXT - 3 || text.chars().count() < MIN_TEXT {
        return Err(format!("Tell us a little more: at least {MIN_TEXT} characters about what happened."));
    }
    let mut text = cut(text, TEXT_MAX);
    let w = s("where")?;
    let where_ = if WHERE.contains(&w.as_str()) { w } else { "launcher".to_string() };
    let page = cut(redact(&s("page")?, own_token).split_whitespace().collect::<Vec<_>>().join(" ").as_str(), PAGE_MAX);
    // the log tail: the newest lines that fit after the player's words
    let lines: Vec<String> = match args.get("log") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        None | Some(Value::Null) => Vec::new(),
        _ => return Err("Bad parameter.".into()),
    };
    if !lines.is_empty() {
        let room = TEXT_MAX.saturating_sub(text.chars().count() + LOG_HEAD.chars().count());
        let mut kept: Vec<String> = Vec::new();
        let mut used = 0;
        for l in lines.iter().rev().take(LOG_LINES) {
            let l = cut(redact(l, own_token).replace('\n', " ").trim(), LOG_LINE_MAX);
            if l.is_empty() {
                continue;
            }
            let n = l.chars().count() + 1;
            if used + n > room {
                break;
            }
            used += n;
            kept.push(l);
        }
        if !kept.is_empty() {
            kept.reverse();
            text.push_str(LOG_HEAD);
            text.push_str(&kept.join("\n"));
        }
    }
    let app: String = format!("Leyline {version}").chars().filter(|c| c.is_alphanumeric() || " _.+-".contains(*c)).take(APP_MAX).collect();
    let mut body = json!({"text": text, "where": where_, "app": app});
    if !page.is_empty() {
        body["page"] = Value::String(page);
    }
    if body.to_string().len() > BODY_MAX {
        return Err("That report is too long. Shorten it a little.".into());
    }
    Ok(body)
}

/// Why a send failed, for the UI: "signed_out", "rate_limited", or a sentence for the player.
pub fn failure(code: Option<u16>, server_error: Option<&str>) -> String {
    match code {
        Some(401) => "signed_out".into(),
        Some(429) => "rate_limited".into(),
        Some(400 | 403) => server_error.map(|e| cut(e, 200)).unwrap_or_else(|| "The bug list didn't take that report.".into()),
        Some(_) => "The bug list isn't taking reports right now. Try again shortly, or use the website's form.".into(),
        None => "Can't reach the Leyline server. Check your connection.".into(),
    }
}

/// Send one report as the signed-in player: {ok, id}, or Err (see `failure`).
pub fn send(dir: &Path, args: &Value, version: &str) -> Result<Value, String> {
    let token = crate::social::load_token(dir).ok_or("signed_out")?;
    let body = request_for(args, version, Some(&token))?;
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(TIMEOUT)
        .timeout_read(TIMEOUT)
        .timeout_write(TIMEOUT)
        .user_agent(concat!("Leyline/", env!("CARGO_PKG_VERSION")))
        .build();
    let r = agent
        .post(&api())
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());
    match r {
        Ok(resp) => {
            let v: Value = resp.into_string().ok().and_then(|b| serde_json::from_str(&b).ok()).unwrap_or(Value::Null);
            let id: String = v.get("id").and_then(Value::as_str).unwrap_or("").chars().filter(char::is_ascii_alphanumeric).take(16).collect();
            Ok(json!({"ok": true, "id": id}))
        }
        Err(ureq::Error::Status(code, resp)) => {
            let err = resp.into_string().ok().and_then(|b| serde_json::from_str::<Value>(&b).ok()).and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string));
            let msg = failure(Some(code), err.as_deref());
            if msg == "signed_out" {
                crate::social::forget(dir);
            }
            Err(msg)
        }
        Err(ureq::Error::Transport(e)) => {
            eprintln!("bugs: {e}"); // for whoever is helping; the player gets a plain line
            Err(failure(None, None))
        }
    }
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;

    /// Send a bug report (Help › Report a bug) as the launcher's signed-in player.
    #[tauri::command]
    pub async fn bug_report(args: Option<Value>) -> Result<Value, String> {
        let args = args.unwrap_or(Value::Null);
        tauri::async_runtime::spawn_blocking(move || send(&crate::settings::app_dir(), &args, env!("CARGO_PKG_VERSION")))
            .await
            .map_err(|e| e.to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOK: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789_-abcde";

    #[test]
    fn user_names_in_paths_are_taken_out() {
        assert_eq!(redact("open /home/orion/.local/share/Leyline/x.json failed", None), "open /home/[user]/.local/share/Leyline/x.json failed");
        assert_eq!(redact("at /Users/Jane Doe", None), "at /Users/[user] Doe");
        assert_eq!(redact(r"C:\Users\Orion\AppData\Roaming\Leyline", None), r"C:\Users\[user]\AppData\Roaming\Leyline");
        assert_eq!(redact(r"D:\\users\\bob\\x", None), r"D:\\users\\[user]\\x");
        assert_eq!(redact(r"C:\Documents and Settings\amy\x", None), r"C:\Documents and Settings\[user]\x");
        assert_eq!(redact(r"C:\Games\Client\Wow.exe", None), r"C:\Games\Client\Wow.exe");
    }

    #[test]
    fn anything_token_like_is_taken_out() {
        assert_eq!(redact("Authorization: Bearer abcdefghijklmnop123", None), "Authorization: Bearer [redacted]");
        assert_eq!(redact("token=abc123 and password: hunter2", None), "token=[redacted] and password: [redacted]");
        assert_eq!(redact("GET https://x.example/a?t=s3cret&page=2", None), "GET https://x.example/a?t=[redacted]&page=2");
        assert_eq!(redact("key 0123456789abcdef0123456789abcdef left", None), "key [redacted] left");
        assert_eq!(redact("mail me at a.b@example.com.", None), "mail me at [email].");
        assert_eq!(redact("realm 192.168.1.20:3724 and fe80::1:2:3", None), "realm [ip]:3724 and [ip]");
        let own = format!("saved {TOK} here");
        assert_eq!(redact(&own, Some(TOK)), "saved [redacted] here");
        // ordinary words stay
        assert_eq!(redact("The quest giver in Northshire: no marker.\nSecond line", None), "The quest giver in Northshire: no marker.\nSecond line");
        assert_eq!(redact("version 0.3.1 build", None), "version 0.3.1 build");
    }

    #[test]
    fn the_request_sends_only_its_fields_within_the_sites_limits() {
        let b = request_for(&json!({"text": "  The Den button does nothing  ", "where": "den", "page": "Launcher › Your Den · realm: Den",
            "token": "x", "url": "https://evil"}), "0.3.1", None).unwrap();
        assert_eq!(b, json!({"text": "The Den button does nothing", "where": "den", "page": "Launcher › Your Den · realm: Den", "app": "Leyline 0.3.1"}));
        // an unknown kind is the launcher's; no page, no page field
        let b = request_for(&json!({"text": "Something broke here", "where": "elsewhere"}), "0.3.1", None).unwrap();
        assert_eq!(b, json!({"text": "Something broke here", "where": "launcher", "app": "Leyline 0.3.1"}));
        // too short (the log never makes up for it), bad types
        assert!(request_for(&json!({"text": "broke", "log": ["a long line of log text here"]}), "1", None).unwrap_err().starts_with("Tell us a little more"));
        assert!(request_for(&json!({"text": ["x"]}), "1", None).is_err());
        assert!(request_for(&json!({"text": "long enough text", "log": "x"}), "1", None).is_err());
        // long text and page are cut to the site's limits
        let b = request_for(&json!({"text": "word ".repeat(600), "page": "p".repeat(300)}), "1", None).unwrap();
        assert!(b["text"].as_str().unwrap().chars().count() <= TEXT_MAX);
        assert_eq!(b["page"].as_str().unwrap().chars().count(), PAGE_MAX);
        // the page line is cleaned too (a custom realm's address)
        let b = request_for(&json!({"text": "Can't connect to it", "page": "realm: 10.0.0.5"}), "1", None).unwrap();
        assert_eq!(b["page"], "realm: [ip]");
    }

    #[test]
    fn the_log_tail_is_cleaned_newest_first_and_fits() {
        let log = vec![
            "old line that should drop when space runs out".to_string(),
            format!("[leyline] read /home/orion/.config/x failed with Bearer {TOK}"),
            "[leyline] fetch ?token=zzz9 failed".to_string(),
        ];
        let b = request_for(&json!({"text": "PLAY does nothing at all", "log": log}), "1", Some(TOK)).unwrap();
        let t = b["text"].as_str().unwrap();
        assert!(t.starts_with("PLAY does nothing at all"));
        assert!(t.contains(LOG_HEAD));
        assert!(t.contains("/home/[user]/.config/x failed with Bearer [redacted]"), "{t}");
        assert!(t.contains("?token=[redacted]"));
        assert!(!t.contains("orion") && !t.contains(TOK) && !t.contains("zzz9"));
        // newest lines win when the text is near the limit
        let long = "x ".repeat(690);
        let b = request_for(&json!({"text": long, "log": log}), "1", None).unwrap();
        let t = b["text"].as_str().unwrap();
        assert!(t.chars().count() <= TEXT_MAX, "{}", t.chars().count());
        assert!(t.contains("?token=[redacted]") && !t.contains("old line"));
        // no room at all: no header either
        let b = request_for(&json!({"text": "y ".repeat(800), "log": log}), "1", None).unwrap();
        assert!(!b["text"].as_str().unwrap().contains("launcher log"));
        // the body stays under the site's form limit even with wide characters
        let b = request_for(&json!({"text": "\u{6F22}".repeat(2000), "page": "\u{1F41B}".repeat(200), "log": vec!["\u{1F41B}".repeat(400); 40]}), "1", None).unwrap();
        assert!(b.to_string().len() <= BODY_MAX);
    }

    #[test]
    fn failures_read_as_states_or_sentences() {
        assert_eq!(failure(Some(401), Some("Signed out.")), "signed_out");
        assert_eq!(failure(Some(429), Some("A lot of reports")), "rate_limited");
        assert_eq!(failure(Some(400), Some("Tell us a little more: at least 10 characters about what happened.")),
            "Tell us a little more: at least 10 characters about what happened.");
        assert!(failure(Some(503), None).contains("website's form"));
        assert!(failure(None, None).starts_with("Can't reach"));
    }

    #[test]
    fn sending_needs_the_launchers_sign_in() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(send(d.path(), &json!({"text": "a real report here"}), "1").unwrap_err(), "signed_out");
    }

    #[test]
    fn the_test_server_must_be_https_or_this_pc() {
        std::env::set_var("LEYLINE_BUGS_API", "http://evil.example/r");
        assert_eq!(api(), API);
        std::env::set_var("LEYLINE_BUGS_API", "http://127.0.0.1:9/wrath/bugs/api/report");
        assert_eq!(api(), "http://127.0.0.1:9/wrath/bugs/api/report");
        std::env::remove_var("LEYLINE_BUGS_API");
    }
}
