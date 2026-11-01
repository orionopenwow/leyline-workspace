//! `den-lite`: runs Den Lite until stopped. The launcher's "Your Den" button would start it, write the client's
//! realmlist (127.0.0.1) and start the game.

use den_lite::{start, Config, Gender};
use std::path::PathBuf;

const USAGE: &str = "den-lite [--dens <folder>] [--bind 127.0.0.1] [--stamps <folder>] [--password <word>] [--name <name>] \
[--gender male|female] [--client <game folder>] [--login-port 3724] [--world-port 8085] [--no-companion] [--verbose]

A tiny local server for the 3.3.5a (12340) client: a white room to build in, on 127.0.0.1.
Any account name (the launcher fills in DEN) logs in with the password (default \"den\"; also DEN_LITE_PASSWORD).
Stamps come from --stamps (default: den-stamps beside the program), else the ones built in.";

fn main() {
    let mut cfg = Config::default();
    if let Ok(p) = std::env::var("DEN_LITE_PASSWORD") {
        cfg.password = p;
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
    match start(cfg) {
        Ok(r) => {
            println!("Den Lite: login {} , world {} . Stop with Ctrl+C.", r.login, r.world);
            loop {
                std::thread::park();
            }
        }
        Err(e) => fail(&format!("can't start: {e}")),
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("den-lite: {msg}\n\n{USAGE}");
    std::process::exit(2)
}
