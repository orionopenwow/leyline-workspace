//! The host console's Network tab (docs/leyline/join-network.md, docs/leyline/be-a-node.md): how this realm shows on
//! the Leyline network, and "Help carry the network" for this PC.
//!
//! * Realm on the network: listed or not, Hero Key sign-in on or off (with Test Hero Key sign-in, the kit's
//!   `wuhost.hktest` dry run), who's online with the opt-in to share names, the declared mods (required or optional),
//!   the house rules, and the logins that came in through the network lately. Read from the kit's `state/` files the
//!   agent writes; written through the kit's own `wuhost.setup` with fixed words and checked values.
//! * Help carry the network: this PC passes other players' game connections through (the kit's community relay,
//!   `docker-compose.volunteer-relay.yml`). It never stores or shares game files. Caps: upload KB/s, players carried,
//!   and "only while this PC is idle and plugged in" (the launcher writes `state/node/pause`; the relay then takes no
//!   new players and shows itself as full). The relay writes `state/node/status.json`: the "carrying N players" line.

use super::docker;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SHORT: Duration = Duration::from_secs(20);
const LONG: Duration = Duration::from_secs(180);
/// The linked agent's container (Link my server, join-network.sh).
pub const LINK_AGENT: &str = "wu-link-agent";
pub const MAX_MODS: usize = 32;

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(p).ok()?).ok()
}

fn state(kit: &Path) -> PathBuf {
    kit.join("state")
}

// ------------------------------------------------------------------------------------------------ the view

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Mod {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default = "yes")]
    pub client: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NodeStatus {
    /// The relay wrote its status in the last minute.
    pub running: bool,
    pub players: u64,
    pub kbps: f64,
    /// Why it takes no new players now: `owner` (paused by this launcher), `banned`, `kill`, or empty.
    pub paused: String,
    pub registered: bool,
    pub healthy: bool,
    pub max_players: u64,
    pub max_kbps: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Network {
    pub set_up: bool,
    pub linked: bool,
    /// The operator's own realm: Hero Key and names are not this window's business.
    pub official: bool,
    pub public: bool,
    /// The relay took the last heartbeat (on the list now).
    pub listed: bool,
    pub connected: bool,
    pub herokey_on: bool,
    /// The website's key is pinned on this kit (join-network / `herokey pin`), or the relay offers Hero Key now.
    pub herokey_key: bool,
    /// The agent can make Hero Key accounts: a kit realm, or a linked server that gave its login database.
    pub herokey_db: bool,
    /// The relay says this realm takes Hero Key sign-in (the agent's last heartbeat).
    pub herokey_live: bool,
    pub share_names: bool,
    pub players: Option<u64>,
    pub who: Vec<String>,
    pub mods: Vec<Mod>,
    pub rules: Vec<String>,
    pub arrivals_hour: u64,
    pub arrivals_day: u64,
    pub node: NodeStatus,
}

pub fn node_status(kit: &Path, now: u64) -> NodeStatus {
    let Some(d) = read_json(&state(kit).join("node").join("status.json")) else { return NodeStatus::default() };
    let n = |k: &str| d[k].as_u64().unwrap_or(0);
    NodeStatus {
        running: now.saturating_sub(n("ts")) <= 60,
        players: n("players"),
        kbps: d["kbps"].as_f64().unwrap_or(0.0),
        paused: d["paused"].as_str().unwrap_or("").chars().take(16).collect(),
        registered: d["registered"].as_bool().unwrap_or(false),
        healthy: d["healthy"].as_bool().unwrap_or(false),
        max_players: d["caps"]["players"].as_u64().unwrap_or(0),
        max_kbps: d["caps"]["kbps"].as_u64().unwrap_or(0),
    }
}

pub fn read(kit: &Path, now: u64) -> Network {
    let st = state(kit);
    let Some(h) = read_json(&st.join("host.json")) else { return Network { node: node_status(kit, now), ..Default::default() } };
    let rs = read_json(&st.join("relay-state.json")).unwrap_or(Value::Null);
    let b = |v: &Value| v.as_bool() == Some(true);
    let linked = b(&h["linked"]);
    let pinned = read_json(&st.join("herokey-site-keys.json")).and_then(|d| d["keys"].as_array().map(|a| !a.is_empty())).unwrap_or(false);
    let mods = h["record"]["mods"].as_array().map(|a| a.iter().filter_map(|m| {
        let id = m["id"].as_str()?.to_string();
        Some(Mod { name: m["name"].as_str().unwrap_or("").to_string(), required: b(&m["required"]),
                   client: m["client"].as_bool().unwrap_or(!id.starts_with("mode:")), id })
    }).collect()).unwrap_or_default();
    Network {
        set_up: true,
        linked,
        official: b(&h["official"]),
        public: h["visibility"].as_str() == Some("public"),
        listed: rs["listing_status"].as_u64() == Some(200) && b(&rs["connected"]),
        connected: b(&rs["connected"]),
        herokey_on: b(&h["herokey"]),
        herokey_key: pinned || b(&rs["herokey"]),
        herokey_db: !linked || st.join("link-db.env").is_file(),
        herokey_live: b(&rs["herokey"]),
        share_names: b(&h["share_names"]),
        players: rs["players"].as_u64(),
        who: rs["who"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).take(50).collect()).unwrap_or_default(),
        mods,
        rules: super::console::rules(kit),
        arrivals_hour: rs["arrivals"]["hour"].as_u64().unwrap_or(0),
        arrivals_day: rs["arrivals"]["day"].as_u64().unwrap_or(0),
        node: node_status(kit, now),
    }
}

// ------------------------------------------------------------------------------------------------ changes

/// A mod id the realm list takes (hosting/relay/wurelay/listing.py MOD_ID_RE): mod:<id>, up:<slug> or mode:<id>.
pub fn mod_id_ok(id: &str) -> bool {
    let low = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    if let Some(r) = id.strip_prefix("mod:") {
        return (3..=32).contains(&r.len()) && r.starts_with(|c: char| c.is_ascii_lowercase()) && r.chars().all(low);
    }
    if let Some(r) = id.strip_prefix("mode:") {
        return (2..=32).contains(&r.len()) && r.starts_with(|c: char| c.is_ascii_lowercase()) && r.chars().all(low);
    }
    if let Some(r) = id.strip_prefix("up:") {
        return (1..=64).contains(&r.len()) && r.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && r.chars().all(|c| low(c) || c == '.' || c == '_');
    }
    false
}

/// The mods list as the kit's `mods set --json` takes it, checked here first.
pub fn mods_json(mods: &[Mod]) -> Result<String, String> {
    if mods.len() > MAX_MODS {
        return Err(format!("A realm lists at most {MAX_MODS} mods."));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for m in mods {
        let id = m.id.trim();
        if !mod_id_ok(id) {
            return Err(format!("\"{}\" is not a mod id: use the id from the Workshop, like up:skada or mod:dalaran-dice.", id.chars().take(40).collect::<String>()));
        }
        if !seen.insert(id.to_string()) {
            continue;
        }
        let name: String = m.name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err("A mod's name: up to 40 characters.".into());
        }
        out.push(serde_json::json!({"id": id, "name": name, "required": m.required, "client": m.client}));
    }
    Ok(Value::Array(out).to_string())
}

/// The kit's setup command for a Network tab change; only these words reach it.
pub fn set_cmd(what: &str, arg: Option<&str>) -> Result<(Vec<String>, String), String> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    Ok(match what {
        "listed-on" => (v(&["visibility", "public"]), "Listed: on the Leyline realm list within a minute.".into()),
        "listed-off" => (v(&["visibility", "private"]), "Not listed: only people with your invite link can join.".into()),
        "herokey-on" => (v(&["herokey", "on"]), "Hero Key sign-in on. Run Test Hero Key sign-in to check every step.".into()),
        "herokey-off" => (v(&["herokey", "off"]), "Hero Key sign-in off: players use your join page again.".into()),
        "herokey-pin" => (v(&["herokey", "pin"]), "Asked the website for its key.".into()),
        "names-on" => (v(&["share-names", "on"]), "The realm list shows who is online (never GMs or bots).".into()),
        "names-off" => (v(&["share-names", "off"]), "The realm list shows only how many are online.".into()),
        "mods" => {
            let mods: Vec<Mod> = serde_json::from_str(arg.unwrap_or("[]")).map_err(|_| "The mods list did not read: try again.".to_string())?;
            (vec!["mods".into(), "set".into(), "--json".into(), mods_json(&mods)?], "Mods saved. The realm list shows them within a minute.".into())
        }
        _ => return Err("That is not a network setting.".into()),
    })
}

pub fn set(kit: &Path, what: &str, arg: Option<&str>) -> Result<String, String> {
    if read_json(&state(kit).join("host.json")).is_none() {
        return Err("Set up your realm first.".into());
    }
    let (cmd, done) = set_cmd(what, arg)?;
    let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
    super::docker_in(kit, docker::tools_args(kit, &c), SHORT)?;
    Ok(done)
}

// ------------------------------------------------------------------------------------------------ Test Hero Key sign-in

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestStep {
    pub id: String,
    pub ok: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestReport {
    pub ok: bool,
    pub steps: Vec<TestStep>,
}

/// Where the dry run runs: beside the realm's database (the kit's agent service), or in the linked agent.
pub fn test_args(kit: &Path, linked: bool) -> Vec<String> {
    if linked {
        return ["exec", LINK_AGENT, "python", "-m", "wuhost.hktest", "--state", "/state", "--json"].iter().map(|s| s.to_string()).collect();
    }
    docker::compose_args(kit, &["run", "--rm", "--no-deps", "wu-agent", "python", "-m", "wuhost.hktest", "--state", "/state", "--json"])
}

/// The dry run's JSON line (wuhost/hktest.py `--json`), from anywhere in docker's output.
pub fn parse_test(out: &str) -> Option<TestReport> {
    let line = out.lines().rev().find(|l| l.trim_start().starts_with('{'))?;
    let d: Value = serde_json::from_str(line.trim()).ok()?;
    let steps = d["steps"].as_array()?.iter().map(|s| TestStep {
        id: s["id"].as_str().unwrap_or("").chars().take(16).collect(),
        ok: s["ok"].as_bool() == Some(true),
        text: s["text"].as_str().unwrap_or("").chars().take(400).collect(),
    }).collect::<Vec<_>>();
    Some(TestReport { ok: d["ok"].as_bool() == Some(true) && !steps.is_empty(), steps })
}

pub fn herokey_test(kit: &Path) -> Result<TestReport, String> {
    let h = read_json(&state(kit).join("host.json")).ok_or("Set up your realm first.")?;
    let args = test_args(kit, h["linked"].as_bool() == Some(true));
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    match docker::run_docker(&a, Some(kit), LONG) {
        None => Err("Docker is not installed (or not on PATH).".into()),
        Some((-2, _)) => Err("The test took too long.".into()),
        Some((_, out)) => parse_test(&out).ok_or_else(|| "The test did not run: is your realm (or its helper) running?".into()),
    }
}

// ------------------------------------------------------------------------------------------------ Help carry the network

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarrySettings {
    pub on: bool,
    /// This PC's public DNS name (a community relay needs TLS).
    #[serde(default)]
    pub public_host: String,
    #[serde(default)]
    pub region: String,
    /// Upload for everyone this PC carries together, KB/s.
    pub kbps: u32,
    pub max_players: u32,
    /// Only while this PC is idle and plugged in.
    pub only_idle: bool,
}

impl Default for CarrySettings {
    fn default() -> Self {
        CarrySettings { on: false, public_host: String::new(), region: String::new(), kbps: 512, max_players: 20, only_idle: true }
    }
}

pub fn carry_path(app_dir: &Path) -> PathBuf {
    app_dir.join("carry.json")
}

pub fn load_carry(app_dir: &Path) -> CarrySettings {
    fs::read(carry_path(app_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn clean_carry(mut s: CarrySettings) -> Result<CarrySettings, String> {
    s.kbps = s.kbps.clamp(64, 100_000);
    s.max_players = s.max_players.clamp(1, 500);
    s.public_host = s.public_host.trim().to_ascii_lowercase();
    if !s.region.is_empty() && !crate::realms::REGIONS.contains(&s.region.as_str()) {
        s.region.clear();
    }
    if s.on && !crate::realms::relay_ok(&s.public_host) {
        return Err("Give this PC's public name (like node.example.org): players reach it by that name.".into());
    }
    Ok(s)
}

pub fn save_carry(app_dir: &Path, s: &CarrySettings) -> Result<(), String> {
    let b = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&carry_path(app_dir), &b).map_err(|e| format!("Could not save: {e}"))
}

/// Pause carrying now? Only with "only while idle and plugged in": the person used this PC in the last
/// five minutes, or it runs on battery. Unknown (another OS) never pauses.
pub fn should_pause(s: &CarrySettings, idle_secs: Option<u64>, on_ac: Option<bool>) -> bool {
    s.on && s.only_idle && (idle_secs.is_some_and(|i| i < 300) || on_ac == Some(false))
}

pub fn pause_file(kit: &Path) -> PathBuf {
    state(kit).join("node").join("pause")
}

/// Write or remove the pause file the relay reads. Returns whether it changed.
pub fn apply_pause(kit: &Path, pause: bool) -> bool {
    let p = pause_file(kit);
    let have = p.is_file();
    if pause && !have {
        let _ = fs::create_dir_all(p.parent().unwrap_or(kit));
        return fs::write(&p, b"paused by the Leyline launcher: this PC is in use or on battery\n").is_ok();
    }
    if !pause && have {
        return fs::remove_file(&p).is_ok();
    }
    false
}

/// The kit's `volunteer-relay` command for these settings.
pub fn carry_cmd(s: &CarrySettings) -> Vec<String> {
    let mut c = vec!["volunteer-relay".to_string(), "--host".into(), s.public_host.clone(), "--max-players".into(),
                     s.max_players.to_string(), "--total-kbps".into(), s.kbps.to_string()];
    if !s.region.is_empty() {
        c.extend(["--region".to_string(), s.region.clone()]);
    }
    c
}

/// Turn carrying on (with these caps) or off. On needs the website's offer and this PC's own yes
/// (`flags::volunteer_allowed`: the owner can stop it for everyone).
pub fn carry_set(kit: &Path, app_dir: &Path, s: CarrySettings, site_allows: bool) -> Result<String, String> {
    let s = clean_carry(s)?;
    if !kit.join(super::VOLUNTEER_COMPOSE).is_file() {
        return Err("This Host pack can't carry the network yet: update the Host pack.".into());
    }
    if !s.on {
        save_carry(app_dir, &s)?;
        super::docker_in(kit, super::volunteer_args(kit, false), LONG)?;
        let _ = super::docker_in(kit, docker::tools_args(kit, &["volunteer-relay", "--off"]), SHORT);
        let _ = crate::p2p::flags::set_local(app_dir, "relay_volunteer", Some(false));
        return Ok("Off. This PC carries nobody now.".into());
    }
    if !site_allows {
        return Err("Helping carry the network is not open yet. It opens for everyone at once.".into());
    }
    crate::p2p::flags::set_local(app_dir, "relay_volunteer", Some(true))?;
    save_carry(app_dir, &s)?;
    let cmd = carry_cmd(&s);
    let c: Vec<&str> = cmd.iter().map(String::as_str).collect();
    super::docker_in(kit, docker::tools_args(kit, &c), SHORT)?;
    super::docker_in(kit, super::volunteer_args(kit, true), LONG)?;
    apply_pause(kit, false);
    Ok(format!("On: this PC helps carry the network as {}, up to {} players.", s.public_host, s.max_players))
}

// ------------------------------------------------------------------------------------------------ idle and power

/// Seconds since the last keyboard or mouse input, where the OS says so.
pub fn idle_secs() -> Option<u64> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct LastInput {
            size: u32,
            time: u32,
        }
        extern "system" {
            fn GetLastInputInfo(p: *mut LastInput) -> i32;
            fn GetTickCount() -> u32;
        }
        let mut li = LastInput { size: std::mem::size_of::<LastInput>() as u32, time: 0 };
        // SAFETY: a plain struct the call fills in; both are user32/kernel32 functions with no other effects
        unsafe {
            if GetLastInputInfo(&mut li) != 0 {
                return Some((GetTickCount().wrapping_sub(li.time) / 1000) as u64);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// On mains power (true), on battery (false), or unknown.
pub fn on_ac() -> Option<bool> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Power {
            ac: u8,
            battery_flag: u8,
            battery_pct: u8,
            saver: u8,
            life: u32,
            full: u32,
        }
        extern "system" {
            fn GetSystemPowerStatus(p: *mut Power) -> i32;
        }
        let mut p = Power { ac: 255, battery_flag: 0, battery_pct: 0, saver: 0, life: 0, full: 0 };
        // SAFETY: the call fills in the struct; no other effects
        unsafe {
            if GetSystemPowerStatus(&mut p) != 0 {
                return match p.ac { 0 => Some(false), 1 => Some(true), _ => None };
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let dir = Path::new("/sys/class/power_supply");
        let mut any = false;
        for e in fs::read_dir(dir).ok()?.flatten() {
            let t = fs::read_to_string(e.path().join("type")).unwrap_or_default();
            if t.trim() == "Mains" {
                any = true;
                if fs::read_to_string(e.path().join("online")).unwrap_or_default().trim() == "1" {
                    return Some(true);
                }
            }
        }
        any.then_some(false)
    }
}

/// One look at the PC: pause or resume the node. Called by the launcher every 30 s while carrying is on.
pub fn tick(kit: &Path, app_dir: &Path) -> bool {
    let s = load_carry(app_dir);
    if !s.on {
        return false;
    }
    apply_pause(kit, should_pause(&s, idle_secs(), on_ac()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit_with(host: &str, relay: &str) -> tempfile::TempDir {
        let k = tempfile::tempdir().unwrap();
        fs::create_dir_all(k.path().join("state")).unwrap();
        fs::write(k.path().join("state").join("host.json"), host).unwrap();
        if !relay.is_empty() {
            fs::write(k.path().join("state").join("relay-state.json"), relay).unwrap();
        }
        k
    }

    #[test]
    fn the_view_from_the_kit_files() {
        let k = kit_with(r#"{"realm_id":"abcdefghijk2345a","visibility":"public","herokey":true,"share_names":true,"linked":true,
            "record":{"mods":[{"id":"up:skada","name":"Skada","required":true},{"id":"mode:minigolf"}]}}"#,
            r#"{"connected":true,"listing_status":200,"players":4,"who":["Grumpy","Aelyn"],"herokey":true,"arrivals":{"hour":2,"day":9}}"#);
        let n = read(k.path(), 1000);
        assert!(n.set_up && n.linked && n.public && n.listed && n.herokey_on && n.herokey_live && n.share_names);
        assert!(!n.herokey_db, "a linked server without its login database can't make Hero Key accounts");
        assert_eq!(n.who, vec!["Grumpy", "Aelyn"]);
        assert_eq!((n.players, n.arrivals_hour, n.arrivals_day), (Some(4), 2, 9));
        assert_eq!(n.mods[0], Mod { id: "up:skada".into(), name: "Skada".into(), required: true, client: true });
        assert!(!n.mods[1].client, "a Leyline Mode runs on the realm");
        fs::write(k.path().join("state").join("link-db.env"), "DB_HOST=x\n").unwrap();
        assert!(read(k.path(), 1000).herokey_db);
        let none = tempfile::tempdir().unwrap();
        assert!(!read(none.path(), 0).set_up);
    }

    #[test]
    fn node_status_and_pause() {
        let k = kit_with("{}", "");
        assert!(!node_status(k.path(), 100).running);
        fs::create_dir_all(k.path().join("state").join("node")).unwrap();
        fs::write(k.path().join("state").join("node").join("status.json"),
                  r#"{"ts":100,"players":3,"kbps":41.5,"paused":null,"registered":true,"healthy":true,"caps":{"players":20,"kbps":512}}"#).unwrap();
        let n = node_status(k.path(), 130);
        assert!(n.running && n.registered && n.healthy);
        assert_eq!((n.players, n.max_players, n.max_kbps, n.paused.as_str()), (3, 20, 512, ""));
        assert!(!node_status(k.path(), 400).running, "a stale status is not running");
        assert!(apply_pause(k.path(), true) && pause_file(k.path()).is_file());
        assert!(!apply_pause(k.path(), true), "already paused");
        assert!(apply_pause(k.path(), false) && !pause_file(k.path()).exists());
    }

    #[test]
    fn pause_only_with_the_idle_rule() {
        let on = CarrySettings { on: true, ..Default::default() };
        assert!(should_pause(&on, Some(30), Some(true)), "in use");
        assert!(should_pause(&on, Some(3600), Some(false)), "on battery");
        assert!(!should_pause(&on, Some(3600), Some(true)));
        assert!(!should_pause(&on, None, None), "unknown never pauses");
        assert!(!should_pause(&CarrySettings { only_idle: false, ..on.clone() }, Some(1), Some(false)));
        assert!(!should_pause(&CarrySettings::default(), Some(1), Some(false)), "off");
    }

    #[test]
    fn carry_settings_are_checked() {
        let s = clean_carry(CarrySettings { on: true, public_host: " Node.Example.org ".into(), kbps: 1, max_players: 9999, region: "mars".into(), only_idle: true }).unwrap();
        assert_eq!((s.public_host.as_str(), s.kbps, s.max_players, s.region.as_str()), ("node.example.org", 64, 500, ""));
        assert!(clean_carry(CarrySettings { on: true, public_host: "not a host".into(), ..Default::default() }).is_err());
        assert!(clean_carry(CarrySettings::default()).is_ok(), "off needs no name");
        let c = carry_cmd(&CarrySettings { region: "eu".into(), ..s });
        assert_eq!(c, ["volunteer-relay", "--host", "node.example.org", "--max-players", "500", "--total-kbps", "64", "--region", "eu"]);
        let d = tempfile::tempdir().unwrap();
        assert!(!load_carry(d.path()).on, "off by default");
        save_carry(d.path(), &CarrySettings { on: true, public_host: "n.example.org".into(), ..Default::default() }).unwrap();
        assert!(load_carry(d.path()).on);
    }

    #[test]
    fn carry_refuses_without_the_site_offer_or_pack() {
        let k = kit_with("{}", "");
        let d = tempfile::tempdir().unwrap();
        let s = CarrySettings { on: true, public_host: "n.example.org".into(), ..Default::default() };
        assert!(carry_set(k.path(), d.path(), s.clone(), true).unwrap_err().contains("update the Host pack"));
        fs::write(k.path().join(super::super::VOLUNTEER_COMPOSE), "name: x\n").unwrap();
        assert!(carry_set(k.path(), d.path(), s, false).unwrap_err().contains("not open yet"));
        assert!(!load_carry(d.path()).on, "nothing saved when refused");
    }

    #[test]
    fn settings_words_and_mods() {
        assert_eq!(set_cmd("listed-on", None).unwrap().0, ["visibility", "public"]);
        assert_eq!(set_cmd("herokey-pin", None).unwrap().0, ["herokey", "pin"]);
        assert_eq!(set_cmd("names-off", None).unwrap().0, ["share-names", "off"]);
        assert!(set_cmd("rm -rf", None).is_err());
        let (c, _) = set_cmd("mods", Some(r#"[{"id":"up:skada","name":"  Skada   meter ","required":true},{"id":"up:skada"},{"id":"mod:pings","client":false}]"#)).unwrap();
        assert_eq!(&c[..3], ["mods", "set", "--json"]);
        let v: Value = serde_json::from_str(&c[3]).unwrap();
        assert_eq!(v, serde_json::json!([{"id":"up:skada","name":"Skada meter","required":true,"client":true},
                                         {"id":"mod:pings","name":"","required":false,"client":false}]));
        for bad in [r#"[{"id":"https://evil.example/x.zip"}]"#, r#"[{"id":"mod:../x"}]"#, r#"[{"id":"mod:ab"}]"#, "nope"] {
            assert!(set_cmd("mods", Some(bad)).is_err(), "{bad}");
        }
        assert!(mod_id_ok("mode:minigolf") && mod_id_ok("up:pfquest") && mod_id_ok("mod:dalaran-dice"));
        let many: Vec<Mod> = (0..33).map(|i| Mod { id: format!("mod:m{i:03}"), ..Default::default() }).collect();
        assert!(mods_json(&many).is_err());
    }

    #[test]
    fn the_test_runs_beside_the_database_and_reads_its_line() {
        let k = kit_with("{}", "");
        assert_eq!(test_args(k.path(), true)[..2], ["exec".to_string(), LINK_AGENT.to_string()]);
        assert!(test_args(k.path(), false).iter().any(|a| a == "wu-agent"));
        let out = "Creating ...\n{\"ok\": true, \"steps\": [{\"id\": \"on\", \"ok\": true, \"text\": \"Hero Key sign-in is switched on.\"}]}\n";
        let r = parse_test(out).unwrap();
        assert!(r.ok && r.steps[0].ok);
        assert!(parse_test("Error: no such container").is_none());
        let r = parse_test("{\"ok\": true, \"steps\": []}").unwrap();
        assert!(!r.ok, "no steps is not a pass");
    }
}
