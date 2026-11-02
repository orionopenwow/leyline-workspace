//! `den-lite`: runs Den Lite until stopped. The launcher's "Your Den" button would start it, write the client's
//! realmlist (127.0.0.1) and start the game.

use den_lite::{start, Config, Gender};
use std::path::PathBuf;

/// `<dens>/den-lite.alive`: "<pid> <unix seconds> <login port>", written every 2 seconds while Den Lite runs.
const ALIVE: &str = "den-lite.alive";
/// `<dens>/den-lite.stop`: when it appears, Den Lite stops (the launcher's Stop, Share with friends).
const STOP: &str = "den-lite.stop";

const USAGE: &str = "den-lite [--dens <folder>] [--bind 127.0.0.1] [--stamps <folder>] [--password <word>] [--name <name>] \
[--gender male|female] [--client <game folder>] [--login-port 3724] [--world-port 8085] [--no-companion] [--no-friends] [--test-control]
den-lite guest add|list|revoke --dens <folder> [--name <name>] [--hours 24] [--verbose]

A tiny local server for the 3.3.5a (12340) client: a white room to build in, on 127.0.0.1.
Any account name (the launcher fills in DEN) logs in with the password (default \"den\"; also DEN_LITE_PASSWORD).
Stamps come from --stamps (default: den-stamps beside the program), else the ones built in.
--test-control (or DEN_LITE_TEST_CONTROL=1): the den-test's commands through the Dens folder (127.0.0.1 only).";

/// `den-lite guest add|list|revoke --dens <folder> ...`: friends' logins (guests.rs), for the launcher's Invite
/// friends. `add [--name <name>] [--hours 24] [--host <host's character>]` prints {"user","pass","name","expires"}
/// once (JSON on one line); `list` prints the live ones without secrets; `revoke <account|all>` takes them back.
fn guest(args: Vec<String>) -> ! {
    let mut dens = PathBuf::from("dens");
    let (mut name, mut hours, mut host, mut rest) = (None, den_lite::guests::DEFAULT_HOURS, "Wren".to_string(), Vec::new());
    let mut it = args.into_iter();
    let cmd = it.next().unwrap_or_default();
    while let Some(a) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--dens" => dens = PathBuf::from(val()),
            "--name" => name = Some(val()),
            "--hours" => hours = val().parse().unwrap_or_else(|_| fail("--hours is a number")),
            "--host" => host = val(),
            _ => rest.push(a),
        }
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let out = match cmd.as_str() {
        "add" => den_lite::guests::add(&dens, name.as_deref(), &host, hours, now).map(|m| serde_json::to_string(&m).unwrap_or_default()),
        "list" => Ok(serde_json::to_string(&den_lite::guests::list(&dens).into_iter().filter(|g| g.live(now))
            .map(|g| serde_json::json!({"user": g.user, "name": g.name, "expires": g.expires})).collect::<Vec<_>>()).unwrap_or_default()),
        "revoke" => den_lite::guests::revoke(&dens, rest.first().map(String::as_str).unwrap_or("all"), now).map(|n| format!("{{\"revoked\":{n}}}")),
        _ => fail("den-lite guest add|list|revoke --dens <folder>"),
    };
    match out {
        Ok(t) => {
            println!("{t}");
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("den-lite: {e}");
            std::process::exit(1)
        }
    }
}

fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().map(String::as_str) == Some("guest") {
        guest(all[1..].to_vec());
    }
    let mut cfg = Config::default();
    if let Ok(p) = std::env::var("DEN_LITE_PASSWORD") {
        cfg.password = p;
    }
    // the launcher's `den-play --test` (an environment variable, so an older Den Lite just ignores it)
    if std::env::var("DEN_LITE_TEST_CONTROL").is_ok_and(|v| v == "1") {
        cfg.test_control = true;
    }
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| fail(&format!("{a} needs a value")));
        match a.as_str() {
            "--dens" => cfg.den_dir = PathBuf::from(val()),
            "--bind" => cfg.bind = val().parse().unwrap_or_else(|_| fail("--bind is an address such as 127.0.0.1")),
            "--stamps" => cfg.stamps_dir = PathBuf::from(val()),
            "--client" => cfg.client = Some(PathBuf::from(val())),
            "--password" => cfg.password = val(),
            "--name" => cfg.hero.name = val(),
            "--gender" => {
                cfg.hero.gender = match val().as_str() {
                    "male" => Gender::Male,
                    "female" => Gender::Female,
                    _ => fail("--gender is male or female"),
                }
            }
            "--login-port" => cfg.login_port = val().parse().unwrap_or_else(|_| fail("a port is a number")),
            "--world-port" => cfg.world_port = val().parse().unwrap_or_else(|_| fail("a port is a number")),
            "--verbose" => cfg.verbose = true,
            "--no-companion" => cfg.companion = false,
            "--no-friends" => cfg.remote_ports = None,
            "--test-control" => cfg.test_control = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            _ => fail(&format!("unknown option {a}")),
        }
    }
    if cfg.password.is_empty() || cfg.password.len() > 16 || !cfg.password.is_ascii() {
        fail("the password is 1 to 16 plain letters, digits or signs");
    }
    if cfg.hero.name.is_empty() || cfg.hero.name.len() > 12 || !cfg.hero.name.chars().all(|c| c.is_ascii_alphabetic()) {
        fail("a name is 1 to 12 letters");
    }
    if !cfg.bind.is_loopback() {
        eprintln!("den-lite: listening on {} (not only this PC): anyone who can reach it logs in with the password", cfg.bind);
    }
    let dens = cfg.den_dir.clone();
    let _ = std::fs::create_dir_all(&dens);
    match start(cfg) {
        Ok(r) => {
            println!("Den Lite: login {} , world {} . Stop with Ctrl+C.", r.login, r.world);
            // The launcher's host/lite.rs knows this Den Lite by its heartbeat (also after the launcher was closed and
            // opened again: then it isn't the launcher's child) and stops it with the stop file.
            let (alive, stop) = (dens.join(ALIVE), dens.join(STOP));
            let _ = std::fs::remove_file(&stop);
            loop {
                let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let _ = std::fs::write(&alive, format!("{} {at} {}\n", std::process::id(), r.login.port()));
                if stop.exists() {
                    let _ = std::fs::remove_file(&stop);
                    let _ = std::fs::remove_file(&alive);
                    std::process::exit(0);
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
        Err(e) => fail(&format!("can't start: {e}")),
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("den-lite: {msg}\n\n{USAGE}");
    std::process::exit(2)
}
