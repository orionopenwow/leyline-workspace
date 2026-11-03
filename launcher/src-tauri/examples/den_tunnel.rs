//! The Den Lite relay tunnel (src/host/litenet.rs) on its own, for the end-to-end test against the real relay
//! (hosting/tests/test_den_lite_tunnel.py). Not shipped; the launcher runs the same code from Invite friends.
//!
//!   cargo run --example den_tunnel --no-default-features -- --relay ws://127.0.0.1:<port>/agent --relay-name relay.test
//!       --dens <Den Lite's Dens folder> --key <key file> --secret <invite secret> [--name "My Den"]
use leyline_lib::host::litenet::{self, Endpoint, Setup};
use std::path::PathBuf;

fn main() {
    let mut a = std::env::args().skip(1);
    let (mut relay, mut name, mut dens, mut key, mut secret, mut den_name) = (String::new(), String::new(), PathBuf::new(), PathBuf::new(), String::new(), "My Den".to_string());
    while let Some(k) = a.next() {
        let v = a.next().unwrap_or_default();
        match k.as_str() {
            "--relay" => relay = v,
            "--relay-name" => name = v,
            "--dens" => dens = PathBuf::from(v),
            "--key" => key = PathBuf::from(v),
            "--secret" => secret = v,
            "--name" => den_name = v,
            _ => panic!("unknown {k}"),
        }
    }
    let rest = relay.strip_prefix("ws://").expect("ws://host:port/path");
    let (hp, path) = rest.split_once('/').unwrap_or((rest, "agent"));
    let (host, port) = hp.split_once(':').expect("host:port");
    let ep = Endpoint { host: host.into(), port: port.parse().unwrap(), tls: false, path: format!("/{path}") };
    let seed = litenet::load_or_create_key(&key).unwrap();
    litenet::start(Setup { endpoint: ep, relay_name: name, seed, name: den_name, invite_secret: secret, den_dir: dens });
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("{}", serde_json::to_string(&litenet::state()).unwrap());
    }
}
