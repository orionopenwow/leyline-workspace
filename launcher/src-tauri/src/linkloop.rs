//! Instant link (owner 3 Nov 01:52: "make linking the launcher way quicker / better, sense clever methods"): while a
//! browser sign-in waits, the launcher answers one question on this PC only, so the website's link page can tell
//! that the launcher on *this* computer is the one showing the code, and link it with no typing (the loopback
//! pattern native apps use for browser sign-in, RFC 8252).
//!
//! `start` listens on 127.0.0.1 (never another address) on a port the system picks, with a random `nonce`; the
//! launcher opens the link page with `&port=<port>&nonce=<nonce>`. The page asks
//! `GET http://127.0.0.1:<port>/leyline-link?nonce=<nonce>` and gets `{"code": "XXXX-XXXX"}`: only with that nonce,
//! only from the website's own pages (Origin: either of its names, addresses.rs), with the CORS and private network
//! headers a browser asks for. A link someone else sent carries their code: this launcher's code differs, so the page
//! falls back to typing it (social.py link_page). The code alone is no secret (the player sees it); the nonce keeps
//! other pages and programs from finding the listener useful. It stops after the sign-in (`stop`) or after
//! `LIFETIME`, whichever comes first.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const LIFETIME: Duration = Duration::from_secs(15 * 60);
const PATH: &str = "/leyline-link";

/// Which listener is current: `stop` and a new `start` move it on, and an older listener then ends.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    pub port: u16,
    pub nonce: String,
}

fn nonce() -> String {
    // 24 random bytes from the system's source (ring, already in the tree), as 32 base64url characters
    let mut b = [0u8; 24];
    let _ = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut b);
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut s = String::with_capacity(32);
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]);
        for i in 0..4 {
            s.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    s
}

/// Start answering for `code` (the code the launcher shows, `XXXX-XXXX`). Err when 127.0.0.1 can't be listened on:
/// then the link page simply asks for the code, as before.
pub fn start(code: &str) -> std::io::Result<Loop> {
    let l = TcpListener::bind(("127.0.0.1", 0))?;
    l.set_nonblocking(true)?;
    let port = l.local_addr()?.port();
    let me = Loop { port, nonce: nonce() };
    let gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let (code, n) = (code.to_string(), me.nonce.clone());
    std::thread::spawn(move || {
        let until = Instant::now() + LIFETIME;
        while GENERATION.load(Ordering::SeqCst) == gen && Instant::now() < until {
            match l.accept() {
                Ok((s, _)) => {
                    let _ = serve(s, &code, &n);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(80)),
            }
        }
    });
    Ok(me)
}

/// The sign-in finished or was cancelled: the listener ends.
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// One request: what goes back, as (status, extra headers, body). Pure, for the tests.
pub fn answer(request: &str, code: &str, nonce: &str) -> (u16, Vec<(String, String)>, String) {
    let mut lines = request.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split(' ');
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let origin = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("origin"))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default();
    if !crate::addresses::is_site_origin(&origin) {
        return (403, vec![], String::new());
    }
    let mut h = vec![
        ("Access-Control-Allow-Origin".to_string(), origin),
        ("Access-Control-Allow-Private-Network".to_string(), "true".to_string()),
        ("Vary".to_string(), "Origin".to_string()),
        ("Cache-Control".to_string(), "no-store".to_string()),
    ];
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != PATH {
        return (404, h, String::new());
    }
    if method == "OPTIONS" {
        h.push(("Access-Control-Allow-Methods".into(), "GET".into()));
        h.push(("Access-Control-Max-Age".into(), "60".into()));
        return (204, h, String::new());
    }
    let given = query.split('&').find_map(|kv| kv.strip_prefix("nonce=")).unwrap_or("");
    let same = given.len() == nonce.len() && given.bytes().zip(nonce.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0;
    if method != "GET" || nonce.is_empty() || !same {
        return (403, h, String::new());
    }
    h.push(("Content-Type".into(), "application/json".into()));
    (200, h, serde_json::json!({ "code": code }).to_string())
}

fn serve(mut s: TcpStream, code: &str, nonce: &str) -> std::io::Result<()> {
    s.set_nonblocking(false)?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while buf.len() < 8192 && !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let (status, headers, body) = answer(&String::from_utf8_lossy(&buf), code, nonce);
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        403 => "Forbidden",
        _ => "Not Found",
    };
    let mut out = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str("\r\n");
    out.push_str(&body);
    s.write_all(out.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://leylinelauncher.com";

    fn req(method: &str, target: &str, origin: &str) -> String {
        format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\n\r\n")
    }

    #[test]
    fn the_code_only_for_the_site_and_the_nonce() {
        let (st, h, body) = answer(&req("GET", "/leyline-link?nonce=abc123", SITE), "BCDF-GHJK", "abc123");
        assert_eq!(st, 200);
        assert_eq!(body, r#"{"code":"BCDF-GHJK"}"#);
        assert!(h.contains(&("Access-Control-Allow-Origin".into(), SITE.into())));
        assert!(h.contains(&("Access-Control-Allow-Private-Network".into(), "true".into())));
        assert_eq!(answer(&req("GET", "/leyline-link?nonce=abc123", "https://play.openvanilla.app"), "C", "abc123").0, 200, "the old name too");
        assert_eq!(answer(&req("GET", "/leyline-link?nonce=wrong1", SITE), "C", "abc123").0, 403, "another nonce");
        assert_eq!(answer(&req("GET", "/leyline-link", SITE), "C", "abc123").0, 403, "no nonce");
        assert_eq!(answer(&req("GET", "/leyline-link?nonce=abc123", "https://evil.example"), "C", "abc123").0, 403, "another site");
        assert_eq!(answer("GET /leyline-link?nonce=abc123 HTTP/1.1\r\n\r\n", "C", "abc123").0, 403, "no origin");
        assert_eq!(answer(&req("GET", "/other?nonce=abc123", SITE), "C", "abc123").0, 404);
        assert_eq!(answer(&req("POST", "/leyline-link?nonce=abc123", SITE), "C", "abc123").0, 403);
        let (st, h, _) = answer(&req("OPTIONS", "/leyline-link?nonce=abc123", SITE), "C", "abc123");
        assert!(st == 204 && h.iter().any(|(k, _)| k == "Access-Control-Allow-Methods"), "the browser's preflight");
    }

    #[test]
    fn a_real_listener_on_this_pc_only_then_gone() {
        let l = start("BCDF-GHJK").unwrap();
        assert!(l.nonce.len() >= 32 && l.port > 0);
        let mut s = TcpStream::connect(("127.0.0.1", l.port)).unwrap();
        s.write_all(req("GET", &format!("/leyline-link?nonce={}", l.nonce), SITE).as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        assert!(out.starts_with("HTTP/1.1 200") && out.ends_with(r#"{"code":"BCDF-GHJK"}"#), "{out}");
        let other = start("XXXX-YYYY").unwrap();
        assert_ne!(other.nonce, l.nonce);
        stop();
        std::thread::sleep(Duration::from_millis(400));
        // the old listeners have ended: a connection is refused or gets nothing
        let gone = TcpStream::connect(("127.0.0.1", l.port)).map(|mut s| {
            let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = s.write_all(b"GET / HTTP/1.1\r\n\r\n");
            let mut b = String::new();
            s.read_to_string(&mut b).map(|_| b.is_empty()).unwrap_or(true)
        });
        assert!(gone.unwrap_or(true));
    }
}
