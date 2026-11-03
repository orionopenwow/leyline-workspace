//! Inviting a friend to Your Den (owner 3 Nov: "make sure inviting a friend works over local, through relay or their
//! own server and give a guide for that"; docs/DEN-INVITE.md). One Invite button, one link, three ways in, tried in
//! this order by the friend's launcher:
//!
//! 1. **Same network** (`lan`): Den Lite's direct listener on this PC's address on its network (labs/den-lite
//!    `--friends-lan`, opened on the first Invite through `<dens>/den-lite.direct-want`). Fastest; no internet needed.
//! 2. **Your own address** (`at`): the same listener reached from the internet, when the player set an address of
//!    their own (a router sending ports 3724 and 8085 to this PC, or their own server): `<dens>/den-lite.direct-host`.
//! 3. **The Leyline relay** (always in the link): the tunnel the launcher keeps to the relay (host/litenet.rs).
//!
//! The link is the relay's own join link with the friend's one-time login after `~~` (litenet.rs), and the other
//! ways after it: `#<secret>~~<account>.<password>~~lan=<ip>:<port>~~at=<host>[:<port>]`. Everything after `#` stays
//! in the browser and the launcher; the relay's join page reads only what comes before the first `~`. Game files
//! never travel by any of the three: each friend plays with their own game folder.
//!
//! Plain std only, so it is unit tested on its own.

use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

/// Den Lite's direct listener's ports (labs/den-lite Config::friends_lan_ports): the game's own login port, so a
/// friend's router rule and realmlist need nothing unusual.
pub const LOGIN_PORT: u16 = 3724;
pub const WORLD_PORT: u16 = 8085;
pub const WANT_FILE: &str = "den-lite.direct-want";
pub const DIRECT_FILE: &str = "den-lite.direct";
pub const HOST_FILE: &str = "den-lite.direct-host";

/// A private (same network) address: 10/8, 172.16/12, 192.168/16, 100.64/10 (carrier, e.g. a VPN mesh), link-local.
pub fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_private() || v.is_link_local() || (v.octets()[0] == 100 && (v.octets()[1] & 0xC0) == 64),
        IpAddr::V6(v) => (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80,
    }
}

/// This PC's address on its network: the one the system would use to reach the internet (a UDP "connect" sends
/// nothing), when it is a private one.
pub fn lan_address() -> Option<IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    private(ip).then_some(ip)
}

/// An address the player typed for "your own address": a host name or IP, optionally `:port`.
pub fn host_ok(h: &str) -> bool {
    let (host, port) = match h.rsplit_once(':') {
        Some((a, p)) if !a.contains(':') => (a, Some(p)),
        _ => (h, None),
    };
    !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && !host.starts_with('-')
        && port.map_or(true, |p| p.parse::<u16>().is_ok_and(|p| p > 0))
}

/// The other ways in a link: (same network "ip:port", own address "host[:port]").
pub fn ways_in(link: &str) -> (Option<String>, Option<String>) {
    let Some((_, frag)) = link.split_once('#') else { return (None, None) };
    let (mut lan, mut at) = (None, None);
    for part in frag.split("~~").skip(2) {
        if let Some(v) = part.strip_prefix("lan=") {
            if v.parse::<SocketAddr>().is_ok_and(|a| private(a.ip())) {
                lan = Some(v.to_string());
            }
        } else if let Some(v) = part.strip_prefix("at=") {
            if host_ok(v) {
                at = Some(v.to_string());
            }
        }
    }
    (lan, at)
}

/// The link with the other ways after the friend's login.
pub fn with_ways(link: &str, lan: Option<&str>, at: Option<&str>) -> String {
    let mut l = link.to_string();
    if let Some(v) = lan {
        l.push_str(&format!("~~lan={v}"));
    }
    if let Some(v) = at.filter(|v| host_ok(v)) {
        l.push_str(&format!("~~at={v}"));
    }
    l
}

/// What `<dens>/den-lite.direct` says: Ok((ip, login port, world port)) or Err(why it is off).
pub fn direct_state(text: &str) -> Result<(IpAddr, u16, u16), String> {
    let t = text.trim();
    if let Some(why) = t.strip_prefix("off") {
        return Err(why.trim().to_string());
    }
    let mut w = t.split_whitespace();
    match (w.next().and_then(|x| x.parse().ok()), w.next().and_then(|x| x.parse().ok()), w.next().and_then(|x| x.parse().ok())) {
        (Some(ip), Some(l), Some(p)) => Ok((ip, l, p)),
        _ => Err("not readable".into()),
    }
}

/// The realmlist for a way: the host alone on the game's own login port, else `host:port`.
pub fn realmlist(addr: &str) -> String {
    match addr.rsplit_once(':') {
        Some((h, p)) if p == LOGIN_PORT.to_string() && !h.contains(':') => h.to_string(),
        _ => addr.to_string(),
    }
}

/// The way the friend's launcher uses: the first that answers (`answers`: a TCP connect within the time given),
/// in the order same network, own address, relay. (way, realmlist or "" for the relay, what was tried).
pub fn pick(lan: Option<&str>, at: Option<&str>, answers: impl Fn(&str, Duration) -> bool) -> (&'static str, String, Vec<String>) {
    let mut tried = Vec::new();
    if let Some(l) = lan {
        if answers(l, Duration::from_millis(800)) {
            return ("lan", realmlist(l), tried);
        }
        tried.push("Same network: no answer (not on the same network as your friend, or a firewall).".into());
    }
    if let Some(a) = at {
        let with_port = if a.rsplit_once(':').is_some_and(|(h, _)| !h.contains(':')) { a.to_string() } else { format!("{a}:{LOGIN_PORT}") };
        if answers(&with_port, Duration::from_millis(2500)) {
            return ("own", realmlist(&with_port), tried);
        }
        tried.push("Their own address: no answer (the router's ports may not be sent to their PC).".into());
    }
    ("relay", String::new(), tried)
}

/// A TCP connect to "host:port" within `t` (the real `answers`).
pub fn connects(addr: &str, t: Duration) -> bool {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok().and_then(|mut a| a.next()).is_some_and(|a| TcpStream::connect_timeout(&a, t).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789";

    #[test]
    fn the_link_carries_every_way() {
        let l = with_ways(BASE, Some("192.168.1.20:3724"), Some("den.example.org"));
        assert!(l.starts_with(BASE));
        assert_eq!(ways_in(&l), (Some("192.168.1.20:3724".into()), Some("den.example.org".into())));
        assert_eq!(ways_in(BASE), (None, None));
        // never a public address as "same network", never a strange host
        assert_eq!(ways_in(&format!("{BASE}~~lan=8.8.8.8:3724~~at=a/b")), (None, None));
        assert_eq!(with_ways(BASE, None, Some("bad host")), BASE);
        assert!(host_ok("81.2.69.160:3725") && host_ok("my-den.example.net") && !host_ok("-x") && !host_ok("x:0") && !host_ok(""));
        assert!(private("192.168.0.4".parse().unwrap()) && private("100.101.1.2".parse().unwrap()) && !private("81.2.69.160".parse().unwrap()));
    }

    #[test]
    fn the_friend_tries_same_network_then_own_address_then_the_relay() {
        let (way, rl, tried) = pick(Some("192.168.1.20:3724"), Some("den.example.org"), |a, _| a.starts_with("192.168."));
        assert_eq!((way, rl.as_str(), tried.len()), ("lan", "192.168.1.20", 0));
        let (way, rl, tried) = pick(Some("192.168.1.20:3724"), Some("den.example.org"), |a, _| a == "den.example.org:3724");
        assert_eq!((way, rl.as_str()), ("own", "den.example.org"));
        assert!(tried[0].starts_with("Same network: no answer"));
        let (way, rl, tried) = pick(Some("192.168.1.20:3724"), Some("1.2.3.4:4000"), |_, _| false);
        assert_eq!((way, rl.as_str(), tried.len()), ("relay", "", 2));
        assert_eq!(realmlist("1.2.3.4:4000"), "1.2.3.4:4000");
        assert_eq!(pick(None, None, |_, _| true).0, "relay");
    }

    #[test]
    fn den_lites_direct_file() {
        assert_eq!(direct_state("192.168.1.20 3724 8085\n").unwrap(), ("192.168.1.20".parse().unwrap(), 3724, 8085));
        assert_eq!(direct_state("off Address already in use (os error 98)").unwrap_err(), "Address already in use (os error 98)");
        assert!(direct_state("junk").is_err());
        // a real listener answers; a closed port doesn't
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let a = l.local_addr().unwrap().to_string();
        assert!(connects(&a, Duration::from_millis(500)));
        drop(l);
        assert!(!connects("127.0.0.1:1", Duration::from_millis(200)));
    }
}
