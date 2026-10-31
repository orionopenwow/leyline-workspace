//! "Already run a server?": look for a game server already on this PC, so the Host page can offer to put it on the
//! Leyline realm list (`leyline link`) with the answers filled in. Read-only, and only what tells where it answers:
//! container names, images and published ports (`docker ps`), the names and folders of running server programs, the
//! port lines of their config files (`WorldServerPort`, `RealmServerPort`: nothing else is kept, never a database
//! login), and whether 127.0.0.1:3724 / :8085 answer (a world server says hello first: SMSG_AUTH_CHALLENGE).
//! The Host kit's own realm (`wu-host-*`) and link agent are never offered.

use serde::Serialize;
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::docker;
use super::HostOs;

pub const AUTH_PORT: u16 = 3724;
pub const WORLD_PORT: u16 = 8085;

/// What a port says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Closed,
    /// Something listens, but not a world server's hello.
    Open,
    /// A world server's first packet: its size (6 on the first two eras, 42 on the third).
    World(u16),
}

/// A server found on this PC.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Found {
    /// docker (its containers), native (its programs), ports (only the game ports answer)
    pub kind: String,
    /// It answers now (the world server said hello, or its containers run).
    pub running: bool,
    /// The world port answered like a world server.
    pub confirmed: bool,
    /// azerothcore, trinitycore, cmangos, vmangos or other.
    pub core: String,
    /// vanilla, tbc or wrath, when it could be told; else empty (the host picks).
    pub client: String,
    /// host:port as this PC reaches them.
    pub auth: String,
    pub world: String,
    /// What was found, in plain words.
    pub what: String,
}

fn core_name(c: &str) -> &'static str {
    match c {
        "azerothcore" => "AzerothCore",
        "trinitycore" => "TrinityCore",
        "cmangos" => "CMaNGOS",
        "vmangos" => "vMaNGOS",
        _ => "a server",
    }
}

/// The core and client from a container's name and image, or a program's name and folder.
fn guess(text: &str) -> (&'static str, &'static str) {
    let t = text.to_ascii_lowercase();
    let core = if t.contains("acore") || t.contains("azeroth") || t.contains("ac-worldserver") || t.contains("ac-authserver") {
        "azerothcore"
    } else if t.contains("trinity") || t.contains("tc-") {
        "trinitycore"
    } else if t.contains("vmangos") {
        "vmangos"
    } else if t.contains("mangos") || t.contains("realmd") {
        "cmangos"
    } else {
        "other"
    };
    let client = if core == "azerothcore" || t.contains("wotlk") || t.contains("wrath") {
        "wrath"
    } else if t.contains("tbc") {
        "tbc"
    } else if core == "vmangos" || t.contains("classic") || t.contains("vanilla") {
        "vanilla"
    } else {
        ""
    };
    (core, client)
}

/// One `docker ps -a --format '{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Ports}}'` line.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Container {
    name: String,
    image: String,
    running: bool,
    /// (host port, container port), tcp only.
    ports: Vec<(u16, u16)>,
}

pub fn ps_args() -> [&'static str; 5] {
    ["ps", "-a", "--no-trunc", "--format", "{{.Names}}\t{{.Image}}\t{{.State}}\t{{.Ports}}"]
}

fn containers(out: &str) -> Vec<Container> {
    docker::plain(out)
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            let name = f.next()?.trim().to_string();
            let image = f.next().unwrap_or("").trim().to_string();
            let running = f.next().unwrap_or("").trim() == "running";
            let ports = f.next().unwrap_or("").split(',').filter_map(|p| {
                // 0.0.0.0:8086->8085/tcp  |  [::]:8086->8085/tcp  |  8085/tcp (not published)
                let (host, inner) = p.trim().split_once("->")?;
                let inner = inner.strip_suffix("/tcp")?;
                let hp = host.rsplit(':').next()?.parse().ok()?;
                Some((hp, inner.parse().ok()?))
            });
            let mut ports: Vec<(u16, u16)> = ports.collect();
            ports.dedup();
            (!name.is_empty()).then_some(Container { name, image, running, ports })
        })
        .collect()
}

fn ours(name: &str) -> bool {
    name.starts_with("wu-host-") || name == "wu-link-agent" || name.starts_with("wu-")
}

fn is_world(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("worldserver") || n.contains("mangosd")
}

fn is_auth(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("authserver") || n.contains("realmd")
}

/// The host port a container publishes for `inner` (else its first published one).
fn published(c: &Container, inner: u16) -> Option<u16> {
    c.ports.iter().find(|(_, i)| *i == inner).or(c.ports.first()).map(|(h, _)| *h)
}

/// `WorldServerPort` / `RealmServerPort` from a config file's text; every other line is skipped unread.
pub fn conf_port(text: &str, key: &str) -> Option<u16> {
    text.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix(key)?.trim_start();
        let v = rest.strip_prefix('=')?.trim().trim_matches('"');
        v.parse().ok().filter(|p: &u16| *p > 0)
    })
}

/// The config files next to a server program (its own folder, or ../etc as AzerothCore's builds have it).
fn conf_files(exe: &Path, names: &[&str]) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else { return Vec::new() };
    let mut v = Vec::new();
    for n in names {
        v.push(dir.join(n));
        if let Some(up) = dir.parent() {
            v.push(up.join("etc").join(n));
        }
    }
    v
}

/// A program's name without its folder or .exe, whichever OS wrote the path.
fn stem(p: &Path) -> String {
    let t = p.to_string_lossy();
    let base = t.rsplit(['/', '\\']).next().unwrap_or("");
    let base = base.strip_suffix(".exe").or_else(|| base.strip_suffix(".EXE")).unwrap_or(base);
    base.to_ascii_lowercase()
}

/// The server programs running, by path where the OS gives one (`ps` / PowerShell), from their listing's text.
pub fn programs(out: &str) -> Vec<PathBuf> {
    let server = |p: &Path| matches!(stem(p).as_str(), "worldserver" | "authserver" | "mangosd" | "realmd");
    out.lines()
        .filter_map(|l| {
            // a whole line is a path (PowerShell's, which may hold spaces); else the program is the first word (ps)
            let whole = PathBuf::from(l.trim().trim_matches('"'));
            if server(&whole) {
                return Some(whole);
            }
            let first = PathBuf::from(l.trim().split_whitespace().next()?.trim_matches('"'));
            server(&first).then_some(first)
        })
        .collect()
}

/// What the scan reads, so it can be tested without a PC.
pub struct Sources<'a> {
    /// `docker ps` output (None: no Docker).
    pub ps: Option<String>,
    /// The running server programs (`programs`).
    pub programs: Vec<PathBuf>,
    /// A port's answer.
    pub probe: &'a dyn Fn(u16) -> Probe,
    /// A config file's text (at most 1 MB), or None.
    pub read: &'a dyn Fn(&Path) -> Option<String>,
}

fn local(p: u16) -> String {
    format!("127.0.0.1:{p}")
}

/// Look, in order: containers, running programs, then the game ports alone.
pub fn scan_with(s: &Sources) -> Option<Found> {
    let mut f = Found::default();
    let mut auth_port = AUTH_PORT;
    let mut world_port = WORLD_PORT;
    let mut seen = false;
    // 1) containers (acore-docker's ac-worldserver / ac-authserver, and the like)
    let cs: Vec<Container> = s.ps.as_deref().map(containers).unwrap_or_default().into_iter().filter(|c| !ours(&c.name)).collect();
    let world_c = cs.iter().find(|c| is_world(&c.name));
    let auth_c = cs.iter().find(|c| is_auth(&c.name));
    if let Some(w) = world_c {
        seen = true;
        let (core, client) = guess(&format!("{} {}", w.name, w.image));
        f.kind = "docker".into();
        f.core = core.into();
        f.client = client.into();
        f.running = w.running && auth_c.map_or(true, |a| a.running);
        world_port = published(w, WORLD_PORT).unwrap_or(WORLD_PORT);
        if let Some(a) = auth_c {
            auth_port = published(a, AUTH_PORT).unwrap_or(AUTH_PORT);
        }
    }
    // 2) a server program running on this PC, and the ports in its config files
    if !seen {
        let world_p = s.programs.iter().find(|p| matches!(stem(p).as_str(), "worldserver" | "mangosd"));
        let auth_p = s.programs.iter().find(|p| matches!(stem(p).as_str(), "authserver" | "realmd"));
        if let Some(w) = world_p {
            seen = true;
            let (core, client) = guess(&w.display().to_string());
            f.kind = "native".into();
            f.core = core.into();
            f.client = client.into();
            f.running = true;
            if let Some(p) = conf_files(w, &["worldserver.conf", "mangosd.conf"]).iter().filter_map(|c| (s.read)(c)).find_map(|t| conf_port(&t, "WorldServerPort")) {
                world_port = p;
            }
            if let Some(p) = conf_files(auth_p.unwrap_or(w), &["authserver.conf", "realmd.conf"]).iter().filter_map(|c| (s.read)(c)).find_map(|t| conf_port(&t, "RealmServerPort")) {
                auth_port = p;
            }
        }
    }
    // 3) the ports: a world server says hello first, and its size tells the era
    let world = (s.probe)(world_port);
    let auth = (s.probe)(auth_port);
    if let Probe::World(size) = world {
        f.confirmed = true;
        f.running = true;
        if f.client.is_empty() || f.core == "other" {
            f.client = if size >= 40 { "wrath".into() } else { f.client.clone() };
        }
    }
    if !seen {
        if auth == Probe::Closed && world == Probe::Closed {
            return None;
        }
        f.kind = "ports".into();
        f.core = "other".into();
        f.running = world != Probe::Closed;
    }
    f.auth = local(auth_port);
    f.world = local(world_port);
    let at = format!("login {auth_port}, world {world_port}");
    f.what = match f.kind.as_str() {
        "docker" if !f.running => format!("{} in Docker, stopped", core_name(&f.core)),
        "docker" => format!("{} in Docker ({at})", core_name(&f.core)),
        "native" => format!("{} running on this PC ({at})", core_name(&f.core)),
        _ if f.confirmed => format!("A server answers on this PC ({at})"),
        _ => format!("Something answers on the game ports ({at}). Is it your server?"),
    };
    Some(f)
}

/// A port on this PC: closed, open, or a world server's hello (read within 600 ms).
pub fn probe(port: u16) -> Probe {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else { return Probe::Closed };
    let _ = s.set_read_timeout(Some(Duration::from_millis(600)));
    let mut head = [0u8; 4];
    match s.read_exact(&mut head) {
        Ok(()) => {
            let size = u16::from_be_bytes([head[0], head[1]]);
            let op = u16::from_le_bytes([head[2], head[3]]);
            if op == 0x1EC { Probe::World(size) } else { Probe::Open }
        }
        Err(_) => Probe::Open,
    }
}

/// The scan on this PC.
pub fn scan() -> Option<Found> {
    let ps = match docker::run_docker(&ps_args(), None, Duration::from_secs(10)) {
        Some((0, out)) => Some(out),
        _ => None,
    };
    let listing = match HostOs::current() {
        HostOs::Windows => docker::run_capture(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command",
              "Get-Process worldserver,authserver,mangosd,realmd -ErrorAction SilentlyContinue | ForEach-Object { $_.Path }"],
            None,
            Duration::from_secs(8),
        ),
        _ => docker::run_capture("ps", &["-eo", "args="], None, Duration::from_secs(5)),
    };
    let progs = listing.map(|(_, out)| programs(&out)).unwrap_or_default();
    let read = |p: &Path| -> Option<String> {
        let f = std::fs::File::open(p).ok()?;
        let mut t = String::new();
        f.take(1024 * 1024).read_to_string(&mut t).ok()?;
        Some(t)
    };
    scan_with(&Sources { ps, programs: progs, probe: &probe, read: &read })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &Path) -> Option<String> {
        None
    }

    #[test]
    fn azerothcore_in_docker_with_its_published_ports() {
        let ps = "acore-docker-ac-database-1\tmysql:8.4\trunning\t3306/tcp, 33060/tcp\n\
                  ac-worldserver\tacore/ac-wotlk-worldserver:master\trunning\t0.0.0.0:8085->8085/tcp, [::]:8085->8085/tcp, 0.0.0.0:7878->7878/tcp\n\
                  ac-authserver\tacore/ac-wotlk-authserver:master\trunning\t0.0.0.0:3724->3724/tcp, [::]:3724->3724/tcp\n";
        let probe = |p: u16| if p == 8085 { Probe::World(42) } else if p == 3724 { Probe::Open } else { Probe::Closed };
        let f = scan_with(&Sources { ps: Some(ps.into()), programs: vec![], probe: &probe, read: &none }).unwrap();
        assert_eq!((f.kind.as_str(), f.core.as_str(), f.client.as_str()), ("docker", "azerothcore", "wrath"));
        assert_eq!((f.auth.as_str(), f.world.as_str(), f.running, f.confirmed), ("127.0.0.1:3724", "127.0.0.1:8085", true, true));
        assert_eq!(f.what, "AzerothCore in Docker (login 3724, world 8085)");
        // published on other host ports
        let ps2 = "ac-worldserver\tacore/ac-wotlk-worldserver:master\trunning\t0.0.0.0:18085->8085/tcp\nac-authserver\tx\trunning\t0.0.0.0:13724->3724/tcp\n";
        let f = scan_with(&Sources { ps: Some(ps2.into()), programs: vec![], probe: &|_| Probe::Closed, read: &none }).unwrap();
        assert_eq!((f.auth.as_str(), f.world.as_str()), ("127.0.0.1:13724", "127.0.0.1:18085"));
        // stopped
        let ps3 = "ac-worldserver\tacore/ac-wotlk-worldserver:master\texited\t\nac-authserver\tx\texited\t\n";
        let f = scan_with(&Sources { ps: Some(ps3.into()), programs: vec![], probe: &|_| Probe::Closed, read: &none }).unwrap();
        assert!(!f.running && f.what.ends_with("stopped"));
    }

    #[test]
    fn the_kits_own_realm_is_never_offered() {
        let ps = "wu-host-worldserver\tghcr.io/orionopenwow/wu-ac-wotlk-worldserver:stable\trunning\t8085/tcp\n\
                  wu-host-authserver\tx\trunning\t3724/tcp\nwu-link-agent\twu-host-tools:1\trunning\t\n";
        assert_eq!(scan_with(&Sources { ps: Some(ps.into()), programs: vec![], probe: &|_| Probe::Closed, read: &none }), None);
    }

    #[test]
    fn a_native_server_and_the_ports_in_its_config() {
        let conf = PathBuf::from("/opt/azerothcore/env/dist/etc/worldserver.conf");
        let aconf = PathBuf::from("/opt/azerothcore/env/dist/etc/authserver.conf");
        let read = |p: &Path| {
            if p == conf {
                Some("LoginDatabaseInfo = \"127.0.0.1;3306;acore;s3cretpw;acore_auth\"\nWorldServerPort = 8086\n".to_string())
            } else if p == aconf {
                Some("RealmServerPort = 3725\nLoginDatabaseInfo = \"127.0.0.1;3306;acore;s3cretpw;acore_auth\"\n".to_string())
            } else {
                None
            }
        };
        let probe = |p: u16| if p == 8086 { Probe::World(42) } else { Probe::Closed };
        let progs = programs("/usr/sbin/sshd -D\n/opt/azerothcore/env/dist/bin/worldserver -c x\n/opt/azerothcore/env/dist/bin/authserver\n");
        assert_eq!(progs.len(), 2);
        let f = scan_with(&Sources { ps: None, programs: progs, probe: &probe, read: &read }).unwrap();
        assert_eq!((f.kind.as_str(), f.world.as_str(), f.auth.as_str(), f.confirmed), ("native", "127.0.0.1:8086", "127.0.0.1:3725", true));
        assert_eq!((f.core.as_str(), f.client.as_str()), ("azerothcore", "wrath"));
        let all = format!("{f:?}");
        assert!(!all.contains("s3cret") && !all.contains("acore_auth"), "a database login never leaves the config file: {all}");
        assert_eq!(conf_port("# WorldServerPort = 1\nWorldServerPort=\"8090\"", "WorldServerPort"), Some(8090));
        // Windows: PowerShell's quoted paths
        let w = programs("C:\\Program Files\\Repack\\mangosd.exe\nC:\\Program Files\\Repack\\realmd.exe\n");
        assert_eq!(w.len(), 2);
        let f = scan_with(&Sources { ps: None, programs: w, probe: &|p| if p == 8085 { Probe::World(6) } else { Probe::Open }, read: &none }).unwrap();
        assert_eq!((f.core.as_str(), f.client.as_str(), f.what.as_str()), ("cmangos", "", "CMaNGOS running on this PC (login 3724, world 8085)"), "the first two eras look alike: the host picks");
    }

    #[test]
    fn ports_alone() {
        let f = scan_with(&Sources { ps: Some(String::new()), programs: vec![], probe: &|p| if p == 8085 { Probe::World(42) } else { Probe::Open }, read: &none }).unwrap();
        assert_eq!((f.kind.as_str(), f.confirmed, f.client.as_str()), ("ports", true, "wrath"));
        // 3724 open and 8085 says something else: never claimed as a world server
        let f = scan_with(&Sources { ps: None, programs: vec![], probe: &|p| if p == 3724 { Probe::Open } else { Probe::Closed }, read: &none }).unwrap();
        assert!(!f.confirmed && f.what.contains("Is it your server?"));
        assert_eq!(scan_with(&Sources { ps: None, programs: vec![], probe: &|_| Probe::Closed, read: &none }), None);
    }

    #[test]
    fn the_world_hello_is_read_from_a_real_socket() {
        use std::io::Write;
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            c.write_all(&[0x00, 0x2A, 0xEC, 0x01]).unwrap();
            c.write_all(&[0u8; 40]).unwrap();
        });
        assert_eq!(probe(port), Probe::World(42));
        t.join().unwrap();
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            c.write_all(b"HTTP/1.1 400 Bad\r\n").unwrap();
        });
        assert_eq!(probe(port), Probe::Open);
        t.join().unwrap();
        let gone = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        assert_eq!(probe(gone), Probe::Closed);
    }
}
