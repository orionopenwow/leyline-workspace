//! Ready for friends? (owner 5 Nov: "user should be able to fully host on their own machine and invite a friend easy
//! 1 step and guide and troubleshoot"). Before the host sends an invite, the launcher checks the same path a friend's
//! game takes and says, line by line, what is green and what to do about what isn't:
//!
//!   1. `server`: the world runs on this PC and its login port answers (a real login hello, not just a connect);
//!   2. `tunnel`: the one outbound connection to the Leyline relay is up (Den Lite: host/litenet.rs; a Docker realm:
//!      its agent, seen through the relay);
//!   3. `relay`: the relay itself says the world is online (`GET /api/realms/<id>`);
//!   4. `path`: a login hello through the relay, the way a friend's game comes in (the relay's per-realm port, after
//!      the same knock the invite link makes), answered by this PC's world: the whole path, end to end;
//!   5. `speed`: how far the relay is (TCP connect time) and the round trip through it, with a plain warning when
//!      friends would feel lag. No ports are needed (the relay works behind any router, a shared CGNAT address too);
//!   6. `sleep`: a PC that sleeps takes the world away from friends (Windows: the power plan's sleep timer);
//!   7. `firewall`: nothing to open for the relay; the same-network way may want Den Lite allowed (Den Lite only).
//!
//! The hello is the game's own first login message for an account that doesn't exist (`LEYLINECHECK`): any server
//! answers it at once with "unknown account", so no account, password or login is used, and no wrong password is
//! counted anywhere. Nothing here prints the invite secret or a friend's login: `diagnostics` is safe to paste.

use serde::Serialize;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// The account name in the self-test hello: no server has it (3 to 16 letters or digits, as accounts are).
pub const CHECK_ACCOUNT: &str = "LEYLINECHECK";
/// The game build the hello names (the one the Wrath worlds take).
pub const BUILD: u16 = 12340;

/// One line of the checklist.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub id: &'static str,
    /// "ok", "warn", "bad", or "skip" (not for this kind of world, or not known here).
    pub state: &'static str,
    pub label: &'static str,
    pub line: String,
    /// One plain sentence: what to do (empty when green).
    pub fix: String,
    /// The fix button the page shows: "start" (start the world and its tunnel again), "power" (the PC's sleep
    /// settings), "firewall" (the firewall's app list), "retest", or "" for none.
    pub action: &'static str,
}

/// The whole checklist.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ready {
    /// "lite" (Your Den on Den Lite) or "docker" (a full realm from the Host pack).
    pub kind: String,
    /// No red line: the invite works now.
    pub ok: bool,
    pub checks: Vec<Check>,
    pub relay: String,
    pub realm_id: String,
    /// TCP connect time to the relay, median of three.
    pub relay_ms: Option<u32>,
    /// The self-test hello's round trip through the relay.
    pub path_ms: Option<u32>,
    pub os: String,
}

/// What was found, before it is judged (`judge` is pure, so the rules are tested without a network).
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub kind: String,
    /// The world's program runs here.
    pub running: bool,
    /// The local login hello: Ok(ms) or why not.
    pub login: Option<Result<u32, String>>,
    /// Den Lite's tunnel: (running, connected, last error). None for a Docker realm.
    pub tunnel: Option<(bool, bool, String)>,
    /// The relay's answer: Ok(online) or why it couldn't be asked.
    pub relay_online: Option<Result<bool, String>>,
    /// The self-test through the relay: Ok(ms) or why not.
    pub path: Option<Result<u32, String>>,
    pub relay_ms: Option<u32>,
    /// The PC sleeps after this many minutes (Some(0): never). None: not known.
    pub sleep_min: Option<u32>,
    /// Den Lite's same-network way: Some(true) open, Some(false) off. None: not asked.
    pub lan: Option<bool>,
    pub os: String,
    pub relay: String,
    pub realm_id: String,
}

fn check(id: &'static str, state: &'static str, label: &'static str, line: impl Into<String>, fix: impl Into<String>, action: &'static str) -> Check {
    Check { id, state, label, line: line.into(), fix: fix.into(), action }
}

/// The relay is slow (ms to connect) or the trip through it is: friends would feel it.
pub const SLOW_RELAY_MS: u32 = 150;
pub const SLOW_PATH_MS: u32 = 400;

/// The checklist from what was found. The order is the path a friend's game takes.
pub fn judge(f: &Facts) -> Ready {
    let lite = f.kind != "docker";
    // a single player world on the Built-in engine (app.rs world_invite: "single" / "vanilla"), or Leyline Solo's
    // first-era world (app.rs classic_invite: "classic"), has the Den's tunnel
    let engine = f.kind == "single" || f.kind == "vanilla" || f.kind == "classic";
    let world = if engine { "Your world" } else if lite { "Your Den" } else { "Your realm" };
    let mut out = Vec::new();
    // 1. the world on this PC
    out.push(match (&f.login, f.running) {
        (Some(Ok(ms)), _) => check("server", "ok", "Your world is running", format!("{world} is running on this PC and its login answers ({ms} ms)."), "", ""),
        (_, false) if engine => check("server", "bad", "Your world is running", "Your world isn't running on this PC.",
            if f.kind == "classic" { "Press Start on the Leyline Solo page, then Invite a friend again." } else { "Press Start on its Single player card, then Invite a friend again." }, "start"),
        (_, false) if lite => check("server", "bad", "Your world is running", "Your Den isn't running on this PC.", "Press Start again: Leyline starts your Den (no game window needed).", "start"),
        (_, false) => check("server", "bad", "Your world is running", "Your realm isn't running on this PC.", "Open the Host page and press Start (Docker Desktop must be running).", "start"),
        (Some(Err(e)), true) => check("server", "bad", "Your world is running", format!("{world} runs but its login doesn't answer: {e}"), "Restart it: press Start again. If it still doesn't answer, restart your PC.", "start"),
        (None, true) => check("server", "skip", "Your world is running", format!("{world} runs (its login wasn't asked)."), "", ""),
    });
    // 2. the tunnel to the relay
    out.push(match &f.tunnel {
        Some((_, true, _)) => check("tunnel", "ok", "Connected to the Leyline relay", "This PC keeps one connection out to the relay: no router settings or open ports needed.", "", ""),
        Some((true, false, e)) => check("tunnel", "bad", "Connected to the Leyline relay",
            if e.is_empty() { "Connecting to the relay… (it should take a few seconds)".to_string() } else { format!("Not connected to the relay yet: {e}") },
            "Check this PC is online. If a VPN, school or work network blocks it, try another network. Then press Check again.", "retest"),
        Some((false, _, _)) => check("tunnel", "bad", "Connected to the Leyline relay", format!("{world} isn't on the Leyline network."), "Press Invite a friend again: it puts it on the network.", "start"),
        None => match &f.relay_online {
            Some(Ok(true)) => check("tunnel", "ok", "Connected to the Leyline relay", "Your realm's agent keeps its connection to the relay: no ports needed.", "", ""),
            _ => check("tunnel", "bad", "Connected to the Leyline relay", "Your realm's agent isn't connected to the relay.", "On the Host page, press Restart. The agent connects within a minute of the realm starting.", "start"),
        },
    });
    // 3. the relay's own view
    out.push(match &f.relay_online {
        Some(Ok(true)) => check("relay", "ok", "The relay sees your world", "The relay lists your world as online (private: only people with your link get in).", "", ""),
        Some(Ok(false)) => check("relay", "bad", "The relay sees your world", "The relay doesn't see your world online yet.", "Wait a few seconds and press Check again. Still red: press Start again.", "retest"),
        Some(Err(e)) => check("relay", "bad", "The relay sees your world", format!("The relay couldn't be asked: {e}"), "Check this PC is online, then press Check again.", "retest"),
        None => check("relay", "skip", "The relay sees your world", "Not asked (no invite link yet).", "", ""),
    });
    // 4. the whole path, as a friend's game takes it
    out.push(match &f.path {
        Some(Ok(ms)) => check("path", "ok", "A friend can reach you", format!("A test login went through the relay to your world and back ({ms} ms): the same way your friend comes in."), "", ""),
        Some(Err(e)) => check("path", "bad", "A friend can reach you", format!("A test login through the relay didn't reach your world: {e}"),
            "Press Start again (it restarts your world's connection), then Check. Still red: Copy diagnostics and send it to us.", "start"),
        None => check("path", "skip", "A friend can reach you", "Not tested (the steps above aren't green yet).", "", ""),
    });
    // 5. speed
    let slow = f.relay_ms.is_some_and(|m| m > SLOW_RELAY_MS) || matches!(f.path, Some(Ok(m)) if m > SLOW_PATH_MS);
    out.push(match (f.relay_ms, slow) {
        (None, _) => check("speed", "skip", "Your connection", "Not measured (the relay didn't answer).", "", ""),
        (Some(ms), false) => check("speed", "ok", "Your connection", format!("About {ms} ms to the relay: good for playing together (each friend adds their own distance)."), "", ""),
        // TODO(owner): the words
        (Some(ms), true) => check("speed", "warn", "Your connection", format!("About {ms} ms to the relay{}: friends may feel lag.",
                match f.path { Some(Ok(p)) => format!(", {p} ms through it"), _ => String::new() }),
            "Use a wired connection instead of Wi-Fi, pause big uploads (cloud backups, streaming), or let the friend with the faster internet host.", ""),
    });
    // 6. sleep
    out.push(match f.sleep_min {
        Some(0) => check("sleep", "ok", "Your PC stays awake", "This PC doesn't go to sleep by itself.", "", ""),
        Some(m) => check("sleep", "warn", "Your PC stays awake", format!("This PC sleeps after {m} minutes without use, and your world stops for your friends while it sleeps."),
            "In your PC's power settings, set Sleep to Never while friends play (or keep using the PC).", "power"),
        None => check("sleep", "skip", "Your PC stays awake", "Keep this computer awake while friends play: your world stops when it sleeps.", "", ""),
    });
    // 7. firewall
    out.push(match (lite, f.lan) {
        (true, Some(false)) => check("firewall", "warn", "Firewall", "Nothing to open for the relay. The faster same-network way is off (friends in your home still get in through the relay).",
            if engine { "To let friends on your Wi-Fi use it: allow Leyline on private networks in your firewall, then press Invite a friend again." }
            else { "To let friends on your Wi-Fi use it: allow Den Lite on private networks in your firewall, then press Invite a friend again." }, "firewall"),
        _ => check("firewall", "ok", "Firewall", "Nothing to open: the relay needs no ports, and works behind any router or a shared (CGNAT) address.", "", ""),
    });
    let ok = !out.iter().any(|c| c.state == "bad");
    Ready { kind: if engine { f.kind.clone() } else if lite { "lite".into() } else { "docker".into() }, ok, checks: out, relay: f.relay.clone(), realm_id: f.realm_id.clone(),
        relay_ms: f.relay_ms, path_ms: f.path.as_ref().and_then(|p| p.as_ref().ok().copied()), os: f.os.clone() }
}

// ------------------------------------------------------------------------------------------------ the probes

/// The game's first login message (the logon challenge, protocol 8) for `account`: what a friend's game sends first.
pub fn logon_hello(account: &str) -> Vec<u8> {
    let name: Vec<u8> = account.bytes().filter(u8::is_ascii_alphanumeric).take(16).map(|b| b.to_ascii_uppercase()).collect();
    let mut b = vec![0x00, 0x08];
    b.extend_from_slice(&((30 + name.len()) as u16).to_le_bytes());
    b.extend_from_slice(&[0x57, 0x6f, 0x57, 0x00]); // the protocol's game name
    b.extend_from_slice(&[3, 3, 5]);
    b.extend_from_slice(&BUILD.to_le_bytes());
    b.extend_from_slice(b"68x\0"); // platform, reversed as the game sends it
    b.extend_from_slice(b"niW\0");
    b.extend_from_slice(b"SUne");
    b.extend_from_slice(&0u32.to_le_bytes()); // time zone
    b.extend_from_slice(&[127, 0, 0, 1]);
    b.push(name.len() as u8);
    b.extend_from_slice(&name);
    b
}

/// The hello sent to `addr` ("host:port") and its answer read: Ok(ms) when a login server answered (any answer to the
/// challenge: "unknown account" is the expected one), else why not in plain words.
pub fn login_answers(addr: &str, t: Duration) -> Result<u32, String> {
    let a = addr.to_socket_addrs().map_err(|_| "its address doesn't resolve".to_string())?.next().ok_or("its address doesn't resolve")?;
    let start = Instant::now();
    let mut s = TcpStream::connect_timeout(&a, t).map_err(|e| match e.kind() {
        std::io::ErrorKind::ConnectionRefused => "nothing listens there".to_string(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => "no answer (timed out)".to_string(),
        _ => format!("couldn't connect ({e})"),
    })?;
    let _ = s.set_read_timeout(Some(t));
    let _ = s.set_write_timeout(Some(t));
    const CLOSED: &str = "the connection closed without an answer (the world behind it isn't answering)";
    s.write_all(&logon_hello(CHECK_ACCOUNT)).map_err(|_| CLOSED.to_string())?;
    let mut got = [0u8; 3];
    let mut n = 0;
    while n < got.len() {
        match s.read(&mut got[n..]) {
            Ok(0) => return Err(if n == 0 { CLOSED.into() } else { "a cut-off answer".into() }),
            Ok(k) => n += k,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => {
                return Err("no answer (timed out): the world behind it isn't answering".into())
            }
            Err(_) => return Err(CLOSED.into()),
        }
    }
    if got[0] != 0x00 {
        return Err("something answered that isn't a login server".into());
    }
    Ok(start.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// TCP connect time to `host:port`, the median of `n` tries (None when none connects).
pub fn connect_ms(host: &str, port: u16, n: usize, t: Duration) -> Option<u32> {
    let a = (host, port).to_socket_addrs().ok()?.next()?;
    let mut v: Vec<u32> = (0..n.max(1))
        .filter_map(|_| {
            let s = Instant::now();
            TcpStream::connect_timeout(&a, t).ok().map(|_| s.elapsed().as_millis().min(u32::MAX as u128) as u32)
        })
        .collect();
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

/// Windows' sleep timer from `powercfg /query SCHEME_CURRENT SUB_SLEEP STANDBYIDLE`, in minutes (0: never). The last
/// two hex values are the plugged-in and the battery setting in seconds, whatever language the text is in.
pub fn sleep_minutes(powercfg: &str) -> Option<u32> {
    let hex: Vec<u32> = powercfg
        .split_whitespace()
        .filter_map(|w| w.strip_prefix("0x").and_then(|h| u32::from_str_radix(h.trim_end_matches(|c: char| !c.is_ascii_hexdigit()), 16).ok()))
        .collect();
    if hex.len() < 2 {
        return None;
    }
    let ac = hex[hex.len() - 2];
    Some(if ac == 0 { 0 } else { (ac / 60).max(1) })
}

/// The sleep timer on this PC (Windows only; None elsewhere or when it can't be read).
#[cfg(windows)]
pub fn sleep_now() -> Option<u32> {
    use std::os::windows::process::CommandExt;
    let o = std::process::Command::new("powercfg")
        .args(["/query", "SCHEME_CURRENT", "SUB_SLEEP", "STANDBYIDLE"])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    sleep_minutes(&String::from_utf8_lossy(&o.stdout))
}

#[cfg(not(windows))]
pub fn sleep_now() -> Option<u32> {
    None
}

/// "https://<relay>/join/<id>[#…]" -> (relay, id), the secret left out (a public realm's link has none).
pub fn link_parts(link: &str) -> Option<(String, String)> {
    let rest = link.trim().strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    let path = path.split('#').next()?;
    let rid = path.strip_prefix("join/")?.trim_end_matches('/');
    let ok = rid.len() == 16 && rid.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        && !host.is_empty() && host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    ok.then(|| (host.to_string(), rid.to_string()))
}

/// An address a friend's launcher may test after joining (`realm_answers`): Leyline's own relay at a port, or a
/// same-network address (the Den's LAN way). Nothing else, so the page can't make the core knock on other servers.
pub fn may_test(addr: &str, relays: &[&str]) -> bool {
    let Some((host, port)) = addr.rsplit_once(':') else { return false };
    if port.parse::<u16>().map_or(true, |p| p == 0) {
        return false;
    }
    relays.contains(&host) || host.parse::<std::net::IpAddr>().is_ok_and(crate::host::denways::private)
}

/// A realm list address with its port ("host" alone is the login port 3724).
pub fn with_port(addr: &str) -> String {
    if addr.contains(':') { addr.to_string() } else { format!("{addr}:3724") }
}

/// Anything that could carry a secret taken out of a line: an invite link's `#…` part, a `~~` login, `password …`.
pub fn redact(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, w) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let w = match w.find('#') {
            Some(k) if w[..k].contains("/join/") || w.contains("~~") => format!("{}#…", &w[..k]),
            _ => w.to_string(),
        };
        let w = match w.find("~~") {
            Some(k) => format!("{}~~…", &w[..k]),
            None => w,
        };
        out.push_str(&w);
    }
    let lower = out.to_ascii_lowercase();
    if let Some(k) = lower.find("password") {
        out.truncate(k + "password".len());
        out.push_str(" …");
    }
    out
}

/// The checklist as plain text to paste in a message to a helper: no invite secret, no logins, no passwords.
pub fn diagnostics(r: &Ready, version: &str) -> String {
    let mut t = format!("Leyline {version} · ready for friends · {} · {}\n", if r.kind == "docker" { "full realm" } else { "Your Den" }, r.os);
    t += &format!("relay {} · world {} · relay {} · through it {}\n", r.relay, r.realm_id,
        r.relay_ms.map(|m| format!("{m} ms")).unwrap_or_else(|| "?".into()), r.path_ms.map(|m| format!("{m} ms")).unwrap_or_else(|| "?".into()));
    for c in &r.checks {
        t += &format!("[{}] {}: {}\n", c.state, c.id, redact(&c.line));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn green() -> Facts {
        Facts { kind: "lite".into(), running: true, login: Some(Ok(2)), tunnel: Some((true, true, String::new())), relay_online: Some(Ok(true)),
            path: Some(Ok(90)), relay_ms: Some(30), sleep_min: Some(0), lan: Some(true), os: "windows".into(), relay: "relay.x".into(), realm_id: "kzdvvj2umnduyauf".into() }
    }
    fn state(r: &Ready, id: &str) -> &'static str {
        r.checks.iter().find(|c| c.id == id).map(|c| c.state).unwrap()
    }

    #[test]
    fn a_single_player_world_is_named_as_one_and_keeps_its_kind() {
        let r = judge(&Facts { kind: "vanilla".into(), ..green() });
        assert!(r.ok);
        assert_eq!(r.kind, "vanilla");
        assert!(r.checks[0].line.starts_with("Your world is running"), "{}", r.checks[0].line);
        let r = judge(&Facts { kind: "single".into(), running: false, login: None, ..green() });
        assert_eq!(state(&r, "server"), "bad");
        assert!(r.checks[0].fix.contains("Single player card"));
        assert!(!r.checks.iter().any(|c| c.line.contains("Your Den")), "never called a Den");
        // Leyline Solo's first-era world: the same checks, its own page in the fix
        let r = judge(&Facts { kind: "classic".into(), ..green() });
        assert!(r.ok && r.kind == "classic");
        let r = judge(&Facts { kind: "classic".into(), running: false, login: None, ..green() });
        assert!(r.checks[0].fix.contains("Leyline Solo page"), "{}", r.checks[0].fix);
        assert!(!r.checks.iter().any(|c| c.line.contains("Your Den")));
    }

    #[test]
    fn all_green_is_ready_and_every_red_line_has_a_fix() {
        let r = judge(&green());
        assert!(r.ok);
        assert_eq!(r.checks.iter().map(|c| c.id).collect::<Vec<_>>(), ["server", "tunnel", "relay", "path", "speed", "sleep", "firewall"]);
        assert!(r.checks.iter().all(|c| c.state == "ok"), "{r:?}");
        let mut f = green();
        f.running = false;
        f.login = None;
        f.tunnel = Some((true, false, "the relay refused".into()));
        f.relay_online = Some(Ok(false));
        f.path = Some(Err("no answer".into()));
        let r = judge(&f);
        assert!(!r.ok);
        for c in r.checks.iter().filter(|c| c.state == "bad") {
            assert!(!c.fix.is_empty(), "{} has a fix", c.id);
        }
        assert_eq!(state(&r, "server"), "bad");
        assert_eq!(r.checks[0].action, "start");
        assert!(r.checks[1].line.contains("the relay refused"));
    }

    #[test]
    fn slow_sleepy_and_firewalled_are_warnings_not_blockers() {
        let mut f = green();
        f.relay_ms = Some(220);
        f.sleep_min = Some(30);
        f.lan = Some(false);
        let r = judge(&f);
        assert!(r.ok, "the invite still works");
        assert_eq!((state(&r, "speed"), state(&r, "sleep"), state(&r, "firewall")), ("warn", "warn", "warn"));
        assert!(r.checks.iter().find(|c| c.id == "sleep").unwrap().line.contains("30 minutes"));
        let mut f = green();
        f.path = Some(Ok(900));
        assert_eq!(state(&judge(&f), "speed"), "warn", "a slow trip through the relay warns too");
    }

    #[test]
    fn a_docker_realm_is_seen_through_the_relay() {
        let mut f = green();
        f.kind = "docker".into();
        f.tunnel = None;
        f.lan = None;
        let r = judge(&f);
        assert!(r.ok);
        assert_eq!(r.kind, "docker");
        f.relay_online = Some(Ok(false));
        let r = judge(&f);
        assert_eq!((state(&r, "tunnel"), state(&r, "relay")), ("bad", "bad"));
    }

    #[test]
    fn the_hello_is_the_games_logon_challenge() {
        let h = logon_hello("leylinecheck");
        assert_eq!(h.len(), 34 + 12);
        assert_eq!(&h[..4], &[0x00, 0x08, 42, 0]);
        assert_eq!(&h[8..13], &[3, 3, 5, 0x34, 0x30]);
        assert_eq!(h[33], 12);
        assert_eq!(&h[34..], b"LEYLINECHECK");
    }

    #[test]
    fn a_login_server_answers_and_a_dead_one_says_why() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut b = [0u8; 46];
            s.read_exact(&mut b).unwrap();
            s.write_all(&[0, 0, 4]).unwrap(); // unknown account
        });
        assert!(login_answers(&addr, Duration::from_secs(3)).is_ok());
        // something that accepts and closes (a relay whose world isn't answering)
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || drop(l.accept()));
        assert!(login_answers(&addr, Duration::from_secs(3)).unwrap_err().contains("without an answer"));
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        drop(l);
        assert!(login_answers(&addr, Duration::from_secs(1)).is_err());
        assert!(connect_ms("127.0.0.1", 1, 1, Duration::from_millis(300)).is_none());
    }

    #[test]
    fn links_and_addresses() {
        assert_eq!(link_parts("https://relay.x/join/kzdvvj2umnduyauf#SECRETSECRETSECRET~~A.B"), Some(("relay.x".into(), "kzdvvj2umnduyauf".into())));
        assert_eq!(link_parts("https://relay.x/join/kzdvvj2umnduyauf"), Some(("relay.x".into(), "kzdvvj2umnduyauf".into())));
        assert_eq!(link_parts("https://relay.x/join/../etc"), None);
        assert_eq!(link_parts("http://relay.x/join/kzdvvj2umnduyauf"), None);
        assert_eq!(with_port("relay.x"), "relay.x:3724");
        assert!(may_test("relay.x:40002", &["relay.x"]));
        assert!(may_test("192.168.1.20:3724", &["relay.x"]));
        assert!(!may_test("evil.example:22", &["relay.x"]));
        assert!(!may_test("8.8.8.8:53", &["relay.x"]));
        assert!(!may_test("relay.x", &["relay.x"]));
        assert_eq!(with_port("relay.x:40002"), "relay.x:40002");
    }

    #[test]
    fn the_sleep_timer_reads_from_powercfg() {
        let t = "Power Setting GUID: 29f6c1db (Sleep after)\n  Minimum Possible Setting: 0x00000000\n  Maximum Possible Setting: 0xffffffff\n  Possible Settings increment: 0x00000001\n  Possible Settings units: Seconds\nCurrent AC Power Setting Index: 0x00000708\nCurrent DC Power Setting Index: 0x00000384\n";
        assert_eq!(sleep_minutes(t), Some(30));
        assert_eq!(sleep_minutes(&t.replace("0x00000708", "0x00000000")), Some(0));
        assert_eq!(sleep_minutes("nothing"), None);
    }

    #[test]
    fn diagnostics_never_carry_a_secret() {
        let mut r = judge(&green());
        r.checks[2].line = "see https://relay.x/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GUEST.PASSWORD23 now".into();
        r.checks[3].line = "At the login screen: account G1, password HUNTER2".into();
        let d = diagnostics(&r, "1.0");
        assert!(!d.contains("ABCDEFGH") && !d.contains("PASSWORD23") && !d.contains("HUNTER2"), "{d}");
        assert!(d.contains("/join/kzdvvj2umnduyauf#…"));
        assert!(d.contains("[ok] server"));
    }
}
