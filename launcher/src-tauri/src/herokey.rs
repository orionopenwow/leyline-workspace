//! Hero Key sign-in to community realms (Beacons) from the launcher: docs/HERO-KEY-SSO.md.
//!
//! Behind the `herokey_sso` flag (`p2p::flags`, OFF by default). With it on, "Sign in with your Hero Key" on a realm
//! card does, in order:
//!
//! 1. asks the website for a ticket for that realm (`POST /wrath/api/social/herokey {realm}`, with the launcher's
//!    own revocable session token from the Leyline sign-in on Home, `crate::social`): a signed assertion for that realm and
//!    a one-time login (`HK...` + a random password) made for this sign-in only;
//! 2. posts the assertion to the realm's relay (`POST https://<relay>/api/realms/<id>/herokey`); the realm's agent
//!    checks the website's signature and sets that login on the player's account there (made on first use). The
//!    password never goes to the relay or the realm, only its SRP verifier inside the signed assertion;
//! 3. writes the account name into the client's `WTF/Config.wtf` (`SET accountName`), points the client at the
//!    realm and starts it. The 3.3.5a client has no way to take a password from outside, so the UI shows the
//!    one-time password for the player to type; it works for 10 minutes and is never saved here.

use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub const REDEEM_TIMEOUT: Duration = Duration::from_secs(10);

/// What the site hands back for one realm, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Ticket {
    pub realm: String,
    pub assertion: Value,
    pub sig: String,
    pub user: String,
    pub pass: String,
    pub redeem: String,
}

/// What the UI gets: the account and the one-time password to type at the login screen, and how long it works.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Login {
    pub realm: String,
    pub user: String,
    pub pass: String,
    pub valid_s: u64,
    /// The hybrid client (crate::hybrid) handed the login to the game: nothing to type. False: show `pass`.
    #[serde(default)]
    pub auto: bool,
    /// Why the hybrid client didn't (with the switch on), in plain words; empty otherwise.
    #[serde(default)]
    pub auto_why: String,
}

/// `HK` + 14 base32 letters (upper case): the realm's account for this player's pseudonym.
pub fn account_ok(s: &str) -> bool {
    s.len() == 16 && s.starts_with("HK") && s[2..].chars().all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

fn pass_ok(s: &str) -> bool {
    (1..=16).contains(&s.len()) && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

/// The redeem address: `https://<relay>/api/realms/<realm>/herokey` on a plain relay host name (or a local test
/// server when LEYLINE_SOCIAL_API points at one).
pub fn redeem_ok(url: &str, realm: &str) -> bool {
    let tail = format!("/api/realms/{realm}/herokey");
    let Some(host) = url.strip_suffix(&tail) else { return false };
    if let Some(h) = host.strip_prefix("https://") {
        return crate::realms::relay_ok(h);
    }
    let local = crate::social::base().starts_with("http://127.0.0.1:");
    local && host.strip_prefix("http://127.0.0.1:").is_some_and(|p| p.parse::<u16>().is_ok())
}

/// Check the site's answer for `realm`.
pub fn parse_ticket(v: &Value, realm: &str) -> Result<Ticket, String> {
    let bad = || "The server's answer didn't look right.".to_string();
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if v.get("ok").and_then(Value::as_bool) != Some(true) || s(v, "realm") != realm {
        return Err(bad());
    }
    let assertion = v.get("assertion").filter(|a| a.is_object()).cloned().ok_or_else(bad)?;
    let sig = s(v, "sig");
    let login = v.get("login").ok_or_else(bad)?;
    let (user, pass, redeem) = (s(login, "user"), s(login, "pass"), s(v, "redeem"));
    let sig_ok = sig.len() == 128 && sig.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    if s(&assertion, "aud") != realm || !sig_ok || !account_ok(&user) || !pass_ok(&pass) || !redeem_ok(&redeem, realm) {
        return Err(bad());
    }
    Ok(Ticket { realm: realm.into(), assertion, sig, user, pass, redeem })
}

/// Step 1: a ticket from the site, as the signed-in player ("signed_out" when the launcher isn't signed in).
pub fn request(app_dir: &Path, realm: &str) -> Result<Ticket, String> {
    if !crate::realms::realm_id_ok(realm) {
        return Err("That is not a realm id.".into());
    }
    let v = crate::social::call(app_dir, "herokey", &serde_json::json!({ "realm": realm }))?;
    parse_ticket(&v, realm)
}

/// Step 2: the realm takes the assertion and sets the one-time login. Returns how long it works, in seconds.
pub fn redeem(t: &Ticket) -> Result<u64, String> {
    let body = serde_json::json!({ "assertion": t.assertion, "sig": t.sig });
    let v = crate::net::post_json(&t.redeem, &body, REDEEM_TIMEOUT).map_err(|e| format!("The realm refused the sign-in: {e}"))?;
    if v.get("ok").and_then(Value::as_bool) != Some(true) || v.get("username").and_then(Value::as_str) != Some(t.user.as_str()) {
        return Err("The realm's answer didn't look right.".into());
    }
    Ok(v.get("valid_s").and_then(Value::as_u64).unwrap_or(0).min(900))
}

/// Steps 1 and 2.
pub fn sign_in(app_dir: &Path, realm: &str) -> Result<Login, String> {
    let t = request(app_dir, realm)?;
    let valid_s = redeem(&t)?;
    Ok(Login { realm: t.realm, user: t.user, pass: t.pass, valid_s, auto: false, auto_why: String::new() })
}

/// The player's own login on the official realm (Wrath Unleashed), for one start of their desktop game (owner 2 Nov:
/// "goes to the loginscreen instead of skipping to character select"). The website already owns one game account per
/// player (`WR000123`, its password kept by the site's sign-in service, the same login the browser game gets) and
/// hands it to this launcher's own token only (`POST /wrath/api/social/game/login`, stream/herokeys.py). It goes
/// straight into the hybrid archive (crate::hybrid, scrubbed when the game closes) and is never written anywhere
/// else, logged or shown. Its Debug never prints the password.
#[derive(Clone, PartialEq)]
pub struct GameLogin {
    pub user: String,
    pub pass: String,
}

impl std::fmt::Debug for GameLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameLogin").field("user", &self.user).field("pass", &"<hidden>").finish()
    }
}

/// Check the site's answer: `{ok, user, pass}`, an account name of 1 to 16 letters and digits, a password the game
/// can take (printable, at most 16).
pub fn parse_game_login(v: &Value) -> Result<GameLogin, String> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (user, pass) = (s("user").to_ascii_uppercase(), s("pass"));
    let user_ok = (1..=16).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_alphanumeric());
    if v.get("ok").and_then(Value::as_bool) != Some(true) || !user_ok || !pass_ok(&pass) {
        return Err("The server's answer didn't look right.".into());
    }
    Ok(GameLogin { user, pass })
}

/// The official realm's login for the player signed in on Home ("signed_out" when the launcher isn't; the site's
/// own words otherwise). Not one of the UI's actions (social::ACTIONS): the page never sees the password.
pub fn official_login(app_dir: &Path) -> Result<GameLogin, String> {
    let v = crate::social::post_as_player(app_dir, &format!("{}/game/login", crate::social::base()), &serde_json::json!({}))
        .map_err(|e| if e.contains("Unknown call") || e.contains("are off here") { "the website can't do that yet".to_string() } else { e })?;
    parse_game_login(&v)
}

/// Config.wtf with `SET accountName "<user>"` (the key keeps its spelling when the line is there; added when not).
pub fn config_wtf_with_account(text: &str, user: &str) -> String {
    config_wtf_with(text, "accountName", user)
}

/// Config.wtf with `SET <key> "<value>"`: the first line of that key (any case) replaced, later ones dropped, or
/// the line added at the end.
pub fn config_wtf_with(text: &str, key: &str, value: &str) -> String {
    let mut hit = false;
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let mut w = l.split_whitespace();
        match (w.next(), w.next()) {
            (Some(a), Some(b)) if a.eq_ignore_ascii_case("SET") && b.eq_ignore_ascii_case(key) => {
                if !hit {
                    lines.push(format!("SET {b} \"{value}\""));
                }
                hit = true;
            }
            _ => lines.push(l.to_string()),
        }
    }
    if !hit {
        lines.push(format!("SET {key} \"{value}\""));
    }
    lines.join("\r\n") + "\r\n"
}

/// Config.wtf without the lines that set `key` to exactly `value` (any case for the key), the rest kept byte for byte.
pub fn config_wtf_without(text: &str, drop: &[(&str, &str)]) -> String {
    text.split_inclusive('\n')
        .filter(|l| {
            let t = l.trim();
            let mut w = t.splitn(3, char::is_whitespace);
            let (Some(set), Some(key), rest) = (w.next(), w.next(), w.next().unwrap_or("")) else { return true };
            let value = rest.trim().trim_matches('"');
            !(set.eq_ignore_ascii_case("SET") && drop.iter().any(|(k, v)| key.eq_ignore_ascii_case(k) && value == *v))
        })
        .collect()
}

/// Take the Den's own login out of Config.wtf (`SET accountName "DEN"`, `SET realmName "Your Den"`: den_launch writes
/// them) before a start anywhere else, so the official realm or a community realm never opens with the Den's account
/// filled in or looks for a realm named Your Den (owner 2 Nov: "swapping realms ... needs to be seamless"). Anything
/// else in the file stays. Returns whether it changed the file.
pub fn clear_login(client_root: &Path, user: &str, realm: &str) -> std::io::Result<bool> {
    let Some(cfg) = config_wtf(client_root) else { return Ok(false) };
    let Ok(old) = std::fs::read_to_string(&cfg) else { return Ok(false) };
    let text = config_wtf_without(&old, &[("accountName", user), ("realmName", realm)]);
    if text == old {
        return Ok(false);
    }
    crate::settings::write_atomic(&cfg, text.as_bytes())?;
    Ok(true)
}

/// An account name Leyline may put on the login screen: a realm's `HK...` name, or a plain local one (the Den's
/// `DEN`, the Docker Den's GM name: 3 to 16 letters and digits). Never anything that could break the line.
pub fn prefill_ok(user: &str) -> bool {
    account_ok(user) || ((3..=16).contains(&user.len()) && user.bytes().all(|b| b.is_ascii_alphanumeric()))
}

fn config_wtf(client_root: &Path) -> Option<std::path::PathBuf> {
    match crate::clients::find_rel(client_root, "WTF/Config.wtf") {
        Some(p) => Some(p),
        None => match crate::clients::find_rel(client_root, "WTF") {
            Some(d) if d.is_dir() => Some(d.join("Config.wtf")),
            _ => None,
        },
    }
}

/// Pre-fill the login screen's account name in the client's `WTF/Config.wtf`. Only a name [`prefill_ok`] takes is
/// written, and only into a client that has a `WTF` folder. Returns whether it wrote. Never a password.
pub fn set_account_name(client_root: &Path, user: &str) -> std::io::Result<bool> {
    set_login(client_root, user, None)
}

/// The account name and, with `realm`, the realm the client goes to after login (`SET realmName`: no realm list
/// when it is the only one). Never a password.
pub fn set_login(client_root: &Path, user: &str, realm: Option<&str>) -> std::io::Result<bool> {
    if !prefill_ok(user) || realm.is_some_and(|r| r.is_empty() || r.len() > 64 || r.chars().any(|c| c.is_control() || c == '"')) {
        return Ok(false);
    }
    let Some(cfg) = config_wtf(client_root) else { return Ok(false) };
    let old = std::fs::read_to_string(&cfg).unwrap_or_default();
    let mut text = config_wtf_with_account(&old, user);
    if let Some(r) = realm {
        text = config_wtf_with(&text, "realmName", r);
    }
    crate::settings::write_atomic(&cfg, text.as_bytes())?;
    let _ = remember_name(&cfg, user);
    Ok(true)
}

/// The account names Leyline itself wrote into this Config.wtf (`Config.wtf.leyline-names`, beside it, one per line,
/// the last few), so a start elsewhere can take them out again: the official account, a community realm's HK name, a
/// friend's Den login, the Docker Den's GM (review 4 Nov: they stayed, and a community realm opened with the player's
/// official account name filled in). The player's own names are never in it.
fn names_path(cfg: &Path) -> std::path::PathBuf {
    let mut n = cfg.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    n.push(".leyline-names");
    cfg.with_file_name(n)
}
const NAMES_KEPT: usize = 8;
fn remember_name(cfg: &Path, user: &str) -> std::io::Result<()> {
    let p = names_path(cfg);
    let mut names: Vec<String> = std::fs::read_to_string(&p).unwrap_or_default().lines().map(str::trim).filter(|l| prefill_ok(l)).map(String::from).collect();
    names.retain(|n| !n.eq_ignore_ascii_case(user));
    names.push(user.to_string());
    let skip = names.len().saturating_sub(NAMES_KEPT);
    crate::settings::write_atomic(&p, (names[skip..].join("\r\n") + "\r\n").as_bytes())
}

/// Take every account name Leyline wrote (see `names_path`) out of Config.wtf: before a start where Leyline signs
/// nobody in, so the login screen opens empty, or with the player's own name. Anything else stays byte for byte.
/// Returns whether it changed the file.
pub fn clear_written_names(client_root: &Path) -> std::io::Result<bool> {
    let Some(cfg) = config_wtf(client_root) else { return Ok(false) };
    let Ok(names) = std::fs::read_to_string(names_path(&cfg)) else { return Ok(false) };
    let names: Vec<&str> = names.lines().map(str::trim).filter(|l| prefill_ok(l)).collect();
    let Ok(old) = std::fs::read_to_string(&cfg) else { return Ok(false) };
    let text: String = old
        .split_inclusive('\n')
        .filter(|l| {
            let mut w = l.trim().splitn(3, char::is_whitespace);
            let (Some(set), Some(key), rest) = (w.next(), w.next(), w.next().unwrap_or("")) else { return true };
            let value = rest.trim().trim_matches('"');
            // account names are the same whatever their case (the game upper-cases what is typed)
            !(set.eq_ignore_ascii_case("SET") && key.eq_ignore_ascii_case("accountName") && names.iter().any(|n| n.eq_ignore_ascii_case(value)))
        })
        .collect();
    if text == old {
        return Ok(false);
    }
    crate::settings::write_atomic(&cfg, text.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::testserver::*;
    use serde_json::json;

    const RID: &str = "abcdefghijk2345a";

    #[test]
    fn the_dens_login_comes_out_before_another_realm() {
        let cfg = "SET realmList \"x\"\r\nSET accountName \"DEN\"\r\nset realmname \"Your Den\"\r\nSET gxWindow \"1\"\r\n";
        assert_eq!(config_wtf_without(cfg, &[("accountName", "DEN"), ("realmName", "Your Den")]), "SET realmList \"x\"\r\nSET gxWindow \"1\"\r\n");
        // the player's own account name and another realm name stay
        let own = "SET accountName \"WR000123\"\nSET realmName \"Wrath Unleashed\"\n";
        assert_eq!(config_wtf_without(own, &[("accountName", "DEN"), ("realmName", "Your Den")]), own);
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("WTF")).unwrap();
        std::fs::write(d.path().join("WTF/Config.wtf"), cfg).unwrap();
        assert!(clear_login(d.path(), "DEN", "Your Den").unwrap());
        assert!(!clear_login(d.path(), "DEN", "Your Den").unwrap(), "nothing left to take out");
        let left = std::fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert!(!left.contains("DEN") && !left.contains("Your Den") && left.contains("gxWindow"), "{left}");
    }

    fn answer(realm: &str, redeem: &str) -> Value {
        json!({"ok": true, "realm": realm, "assertion": {"aud": realm, "sub": "x"}, "sig": "ab".repeat(64),
               "login": {"user": "HKABCDEFGHIJKLMN", "pass": "Q7W8E9R2T3Y4U5I6"}, "redeem": redeem})
    }

    #[test]
    fn the_ticket_is_checked() {
        let good = answer(RID, &format!("https://relay.openvanilla.app/api/realms/{RID}/herokey"));
        let t = parse_ticket(&good, RID).unwrap();
        assert_eq!((t.user.as_str(), t.pass.as_str()), ("HKABCDEFGHIJKLMN", "Q7W8E9R2T3Y4U5I6"));
        // another realm, another realm's redeem address, a relay that isn't a plain host, a login that isn't an HK
        // account, a bad signature field: refused
        assert!(parse_ticket(&good, "qrstuvwxyz234567").is_err());
        assert!(parse_ticket(&answer(RID, "https://relay.openvanilla.app/api/realms/qrstuvwxyz234567/herokey"), RID).is_err());
        assert!(parse_ticket(&answer(RID, &format!("https://evil.example:8443/api/realms/{RID}/herokey")), RID).is_err());
        assert!(parse_ticket(&answer(RID, &format!("http://relay.openvanilla.app/api/realms/{RID}/herokey")), RID).is_err());
        let mut v = good.clone();
        v["login"]["user"] = json!("WR000123");
        assert!(parse_ticket(&v, RID).is_err());
        let mut v = good.clone();
        v["assertion"]["aud"] = json!("qrstuvwxyz234567");
        assert!(parse_ticket(&v, RID).is_err());
        let mut v = good.clone();
        v["sig"] = json!("zz");
        assert!(parse_ticket(&v, RID).is_err());
        let mut v = good;
        v["ok"] = json!(false);
        assert!(parse_ticket(&v, RID).is_err());
        assert!(account_ok("HKABCDEFGHIJ2345") && !account_ok("HKabcdefghij2345") && !account_ok("HK1"));
    }

    #[test]
    fn redeem_posts_the_assertion_only() {
        let base = serve_routes(vec![(
            "/api/realms/abcdefghijk2345a/herokey",
            "200 OK",
            br#"{"ok":true,"username":"HKABCDEFGHIJKLMN","valid_s":600}"#.to_vec(),
        )]);
        let t = Ticket {
            realm: RID.into(),
            assertion: json!({"aud": RID}),
            sig: "ab".repeat(64),
            user: "HKABCDEFGHIJKLMN".into(),
            pass: "Q7W8E9R2T3Y4U5I6".into(),
            redeem: format!("{base}/api/realms/{RID}/herokey"),
        };
        assert_eq!(redeem(&t).unwrap(), 600);
        let wrong = Ticket { user: "HKZZZZZZZZZZZZZZ".into(), ..t.clone() };
        assert!(redeem(&wrong).is_err(), "the realm must have set this account's login");
        let refused = serve_routes(vec![]);
        let t2 = Ticket { redeem: format!("{refused}/api/realms/{RID}/herokey"), ..t };
        assert!(redeem(&t2).unwrap_err().contains("refused"));
    }

    #[test]
    fn the_official_realms_login_is_checked_and_never_printed() {
        let l = parse_game_login(&json!({"ok": true, "user": "wr000123", "pass": "Q7W8E9R2T3Y4U5I6"})).unwrap();
        assert_eq!((l.user.as_str(), l.pass.as_str()), ("WR000123", "Q7W8E9R2T3Y4U5I6"));
        let shown = format!("{l:?}");
        assert!(shown.contains("WR000123") && !shown.contains("Q7W8"), "{shown}");
        for bad in [json!({"ok": false, "user": "WR1", "pass": "x"}), json!({"ok": true, "user": "WR 1", "pass": "x"}),
            json!({"ok": true, "user": "WR1", "pass": ""}), json!({"ok": true, "user": "WR1", "pass": "a b"}),
            json!({"ok": true, "user": "WR1", "pass": "x".repeat(17)}), json!({"ok": true, "user": "W".repeat(17), "pass": "x"}),
            json!({"ok": true, "user": "WR1"})] {
            assert!(parse_game_login(&bad).is_err(), "{bad}");
        }
        let d = tempfile::tempdir().unwrap();
        assert_eq!(official_login(d.path()).unwrap_err(), "signed_out", "no sign-in on Home: nothing asked");
    }

    #[test]
    fn signed_out_and_bad_ids() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(request(d.path(), RID).unwrap_err(), "signed_out");
        assert!(request(d.path(), "../x").is_err());
    }

    #[test]
    fn account_name_in_config_wtf() {
        assert_eq!(config_wtf_with_account("", "HKABCDEFGHIJKLMN"), "SET accountName \"HKABCDEFGHIJKLMN\"\r\n");
        assert_eq!(
            config_wtf_with_account("SET gxApi \"d3d9\"\r\nSET accountName \"OLD\"\r\nSET accountname \"DUP\"\r\n", "HKABCDEFGHIJKLMN"),
            "SET gxApi \"d3d9\"\r\nSET accountName \"HKABCDEFGHIJKLMN\"\r\n"
        );
        let d = tempfile::tempdir().unwrap();
        assert!(!set_account_name(d.path(), "HKABCDEFGHIJKLMN").unwrap(), "no WTF folder: nothing written");
        std::fs::create_dir(d.path().join("WTF")).unwrap();
        assert!(!set_account_name(d.path(), "evil\" x").unwrap(), "only a plain account name");
        assert!(!set_account_name(d.path(), "DE").unwrap() && !set_account_name(d.path(), "a-b-c").unwrap());
        assert!(set_account_name(d.path(), "HKABCDEFGHIJKLMN").unwrap());
        let text = std::fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert!(text.contains("SET accountName \"HKABCDEFGHIJKLMN\"") && !text.contains("Q7W8"), "never a password");
        // the local Den's account (Den Lite's DEN, the Docker Den's GM name) and its realm
        assert!(set_login(d.path(), "DEN", Some("Your Den")).unwrap());
        let text = std::fs::read_to_string(d.path().join("WTF/Config.wtf")).unwrap();
        assert!(text.contains("SET accountName \"DEN\"\r\n") && text.contains("SET realmName \"Your Den\"\r\n"), "{text}");
        assert!(!text.to_lowercase().contains("password"));
        assert!(!set_login(d.path(), "DEN", Some("a\"b")).unwrap(), "a realm name that would break the line");
        assert!(set_account_name(d.path(), "Builder7").unwrap());
    }

    #[test]
    fn the_names_leyline_wrote_come_out_before_a_start_that_signs_nobody_in() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("WTF")).unwrap();
        let cfg = d.path().join("WTF/Config.wtf");
        assert!(!clear_written_names(d.path()).unwrap(), "nothing written yet: nothing to take out");
        // the official realm's account, then a community realm's HK name: both remembered
        assert!(set_account_name(d.path(), "WR000123").unwrap());
        assert!(set_account_name(d.path(), "HKABCDEFGHIJKLMN").unwrap());
        let names = std::fs::read_to_string(names_path(&cfg)).unwrap();
        assert_eq!(names, "WR000123\r\nHKABCDEFGHIJKLMN\r\n");
        std::fs::write(&cfg, "SET gxWindow \"1\"\r\nSET accountName \"wr000123\"\r\nSET realmList \"x\"\r\n").unwrap();
        assert!(clear_written_names(d.path()).unwrap(), "the official name, whatever its case");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), "SET gxWindow \"1\"\r\nSET realmList \"x\"\r\n");
        assert!(!clear_written_names(d.path()).unwrap(), "nothing left");
        // the player's own name stays
        std::fs::write(&cfg, "SET accountName \"MYOWNNAME\"\r\n").unwrap();
        assert!(!clear_written_names(d.path()).unwrap());
        assert!(std::fs::read_to_string(&cfg).unwrap().contains("MYOWNNAME"));
        // only the last few are kept, each once
        for i in 0..12 {
            set_account_name(d.path(), &format!("NAME{i:02}")).unwrap();
        }
        set_account_name(d.path(), "NAME05").unwrap();
        let names = std::fs::read_to_string(names_path(&cfg)).unwrap();
        assert_eq!(names.lines().count(), NAMES_KEPT, "{names}");
        assert!(names.ends_with("NAME11\r\nNAME05\r\n") && !names.contains("NAME00"), "{names}");
    }
}
