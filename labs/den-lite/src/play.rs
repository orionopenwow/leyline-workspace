//! `.den mode` and `.den mods`: what Den Lite can honestly say about testing game modes and mods on this PC.
//!
//! Den Lite runs no game mode (no creatures but the companion, no combat, spells, quests or loot), so `.den mode`
//! lists Leyline's modes (the launcher's Modes page, `stream/playmodes.json`, and the Host page's templates,
//! `stream/modepacks.json`, both built in as the launcher builds them in) and says how to run one on this PC: the
//! full Den / Host realm, from its Host template. `.den mods` lists the addons the game itself loaded (the list the
//! client sends when it logs in, CMSG_AUTH_SESSION) and, given the game folder (`--client`), what the launcher's mod
//! manager put in it (`<game>/Leyline/mods.json`, launcher/src-tauri/src/mods.rs `Manifest`).

use serde_json::Value;
use std::io::Read;
use std::path::Path;

pub const PLAYMODES: &str = include_str!("../../../stream/playmodes.json");
pub const MODEPACKS: &str = include_str!("../../../stream/modepacks.json");

/// One mode as `.den mode` shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub line: String,
    /// "template" (a Host template) or "mode" (the Modes page).
    pub source: &'static str,
    pub live: bool,
    /// Templates: the ruleset and the scripts; modes: how it starts (start.go / host / cmd / path).
    pub profile: Option<String>,
    pub scripts: Vec<String>,
    pub mods: Vec<String>,
    pub start: Option<Value>,
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn list(v: &Value, k: &str) -> Vec<String> {
    v.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

/// The templates first (they are what runs on a PC), then the Modes page's modes.
pub fn modes() -> Vec<Mode> {
    let mut out = Vec::new();
    let read = |t: &str| serde_json::from_str::<Value>(t).ok().and_then(|v| v.get("modes").and_then(Value::as_array).cloned()).unwrap_or_default();
    for m in read(MODEPACKS) {
        out.push(Mode {
            id: s(&m, "id"),
            name: s(&m, "name"),
            line: s(&m, "line"),
            source: "template",
            live: s(&m, "status") == "live",
            profile: m.get("profile").and_then(Value::as_str).map(str::to_string),
            scripts: list(&m, "scripts"),
            mods: list(&m, "mods"),
            start: None,
        });
    }
    for m in read(PLAYMODES) {
        out.push(Mode {
            id: s(&m, "id"),
            name: s(&m, "title"),
            line: s(&m, "line"),
            source: "mode",
            live: s(&m, "status") == "live",
            profile: None,
            scripts: Vec::new(),
            mods: Vec::new(),
            start: m.get("start").cloned(),
        });
    }
    out.retain(|m| !m.id.is_empty() && !m.name.is_empty());
    out
}

/// `.den mode`: the ids, in two lines.
pub fn list_lines() -> Vec<String> {
    let all = modes();
    let ids = |src: &str, live: bool| {
        all.iter().filter(|m| m.source == src && m.live == live && m.id != "den").map(|m| m.id.as_str()).collect::<Vec<_>>().join(", ")
    };
    // the ones that run now, then the ones still coming (smoke O7: "coming" and this Den itself aren't "run on this PC")
    let soon: Vec<String> = [ids("template", false), ids("mode", false)].into_iter().filter(|x| !x.is_empty()).collect();
    let mut modes_line = format!("Game modes (Leyline's Modes page): {}.", ids("mode", true));
    if !soon.is_empty() {
        modes_line.push_str(&format!(" Coming: {}.", soon.join(", ")));
    }
    vec![
        format!("Host templates (run on this PC): {}.", ids("template", true)),
        modes_line,
        ".den mode <id> says what one is and how to try it here. Den Lite itself only builds: no combat, spells or quests.".into(),
    ]
}

fn host_line(t: &Mode) -> String {
    let rules = t.profile.as_deref().map(|p| format!(" (ruleset {p}{})", if t.scripts.is_empty() { String::new() } else { format!("; {}", t.scripts.join(", ")) })).unwrap_or_default();
    format!("Try it on this PC: Leyline › Host › Host a mode › {} › Host{rules}. It runs as a realm in Docker on your PC; your Den files go with it.", t.name)
}

/// `.den mode <id>`: what it is and how to run it, a few lines.
pub fn explain(id: &str) -> Vec<String> {
    let all = modes();
    let id = id.trim().to_lowercase();
    let Some(m) = all.iter().find(|m| m.id == id) else {
        return vec![format!("No mode called '{id}'. .den mode lists them.")];
    };
    let mut out = vec![format!("{}: {}", m.name, m.line)];
    if !m.live {
        out.push(format!("{} is coming: there is nothing to run yet.", m.name));
        return out;
    }
    let templates: Vec<&Mode> = all.iter().filter(|t| t.source == "template" && t.live).collect();
    if m.source == "template" {
        if m.id == "den" {
            out.push("That's this one: your Den. The full Den (Leyline › Host › Host a mode › My Den › Host) adds friends through the invite link and builds from your assistant's plans.".into());
        } else {
            out.push("Den Lite can't run it: it has no creatures, combat, spells or quests.".into());
            out.push(host_line(m));
        }
        return out;
    }
    let start = m.start.clone().unwrap_or(Value::Null);
    let go = s(&start, "go");
    let host = s(&start, "host");
    let cmd = s(&start, "cmd");
    // the template that runs it: one that carries the mode (mods "mode:<id>"), or the ruleset it names
    let t = templates
        .iter()
        .find(|t| !go.is_empty() && t.mods.iter().any(|x| *x == go))
        .or_else(|| templates.iter().find(|t| !host.is_empty() && (t.id == host || t.profile.as_deref() == Some(host.as_str()))));
    if let Some(t) = t {
        out.push(format!("Den Lite can't run it. It comes with the {} template.", t.name));
        out.push(host_line(t));
    } else if !cmd.is_empty() {
        out.push(format!("A part of a realm, in game: {cmd} on a realm that has it on. Den Lite can't run it."));
    } else {
        out.push(format!("Den Lite can't run it: Leyline › Modes › {} starts it.", m.name));
    }
    out
}

/// The addon names in the client's CMSG_AUTH_SESSION list (u32 size, then zlib: u32 count, then each: name,
/// has-key byte, crc, extra crc), plain names only, up to 512.
pub fn addon_names(blob: &[u8]) -> Vec<String> {
    use flate2::read::ZlibDecoder;
    let mut out = Vec::new();
    if blob.len() < 4 {
        return out;
    }
    let mut b = Vec::new();
    if ZlibDecoder::new(&blob[4..]).take(64 * 1024).read_to_end(&mut b).is_err() || b.len() < 4 {
        return out;
    }
    let n = (u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).min(512);
    let mut at = 4;
    for _ in 0..n {
        let Some(end) = b[at..].iter().position(|&c| c == 0).map(|e| at + e) else { break };
        let name = String::from_utf8_lossy(&b[at..end]).to_string();
        at = end + 1 + 1 + 4 + 4;
        if at > b.len() {
            break;
        }
        if !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-. ".contains(c)) {
            out.push(name);
        }
    }
    out
}

/// `.den mods`: the addons the game loaded, and the launcher's mod list when the game folder is known.
pub fn mods_lines(addons: &[String], client: Option<&Path>) -> Vec<String> {
    let (stock, own): (Vec<&String>, Vec<&String>) = addons.iter().partition(|a| a.to_ascii_lowercase().starts_with("blizzard_"));
    let mut out = Vec::new();
    if own.is_empty() {
        out.push(format!("Your game loaded no addons of its own ({} built-in ones).", stock.len()));
    } else {
        let names: Vec<&str> = own.iter().take(20).map(|s| s.as_str()).collect();
        let more = if own.len() > 20 { format!(" and {} more", own.len() - 20) } else { String::new() };
        out.push(format!("Your game loaded {} addons: {}{more} (and {} built-in ones).", own.len(), names.join(", "), stock.len()));
    }
    out.push("Addons run in your game as on any realm, so your Den is a quiet place to try them: /reload after changing one. Mods with realm parts need the full Den (.den mode den).".into());
    match client {
        None => out.push("Start your Den from the Leyline launcher to see its mods for this game here too.".into()),
        Some(dir) => {
            let p = dir.join("Leyline").join("mods.json");
            let v: Option<Value> = std::fs::read_to_string(&p).ok().filter(|t| t.len() < 4 * 1024 * 1024).and_then(|t| serde_json::from_str(&t).ok());
            let mods = v.as_ref().and_then(|v| v.get("mods")).and_then(Value::as_array).cloned().unwrap_or_default();
            if mods.is_empty() {
                out.push("Leyline's mod manager hasn't put any mods in this game.".into());
            } else {
                let rows: Vec<String> = mods
                    .iter()
                    .take(20)
                    .map(|m| {
                        let ver = s(m, "version");
                        let on = m.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                        let files = !list(m, "patches").is_empty() || !list(m, "natives").is_empty();
                        format!("{}{}{}{}", s(m, "name"), if ver.is_empty() { String::new() } else { format!(" {ver}") }, if on { "" } else { " (off)" }, if files { " (game files: restart to change)" } else { "" })
                    })
                    .collect();
                out.push(format!("Leyline's mods in this game: {}.", rows.join(", ")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modes_are_listed_and_explained() {
        let all = modes();
        assert!(all.iter().any(|m| m.id == "minigolf-night" && m.source == "template" && m.profile.as_deref() == Some("dalaran")));
        assert!(all.iter().any(|m| m.id == "minigolf" && m.source == "mode"));
        let l = list_lines();
        assert!(l[0].contains("duel-ladder") && l[1].contains("minigolf"), "{l:?}");
        let run = l[0].split(':').nth(1).unwrap();
        assert!(!run.contains("survivors") && !run.split(", ").any(|x| x.trim_matches(['.', ' ']) == "den"), "only what runs: {l:?}");
        assert!(l[1].contains("Coming:") && l[1].split("Coming:").nth(1).unwrap().contains("survivors"), "{l:?}");
        let golf = explain("Minigolf");
        assert!(golf[1].contains("Minigolf night template") && golf[2].contains("Host a mode › Minigolf night › Host (ruleset dalaran"), "{golf:?}");
        let duel = explain("duel-ladder");
        assert!(duel[1].contains("can't run") && duel[2].contains("ruleset brawl; ov_duels.lua"), "{duel:?}");
        assert!(explain("brawl")[1].contains("Duel ladder"), "start.host brawl: the brawl ruleset's template");
        assert!(explain("arena-1v1")[1].contains(".hub arena"));
        assert!(explain("survivors")[1].contains("coming"));
        assert!(explain("den")[1].starts_with("That's this one"));
        assert!(explain("nope")[0].starts_with("No mode called 'nope'"));
    }

    #[test]
    fn the_games_addons_and_the_launchers_mods() {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;
        let mut plain = 3u32.to_le_bytes().to_vec();
        for n in ["Blizzard_AuctionUI", "LeylineDen", "Questie"] {
            plain.extend_from_slice(n.as_bytes());
            plain.push(0);
            plain.push(1);
            plain.extend_from_slice(&[0; 8]);
        }
        plain.extend_from_slice(&0u32.to_le_bytes());
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&plain).unwrap();
        let mut blob = (plain.len() as u32).to_le_bytes().to_vec();
        blob.extend(z.finish().unwrap());
        let names = addon_names(&blob);
        assert_eq!(names, ["Blizzard_AuctionUI", "LeylineDen", "Questie"]);
        let l = mods_lines(&names, None);
        assert!(l[0].starts_with("Your game loaded 2 addons: LeylineDen, Questie (and 1 built-in ones)"), "{l:?}");
        assert!(l[2].contains("Leyline launcher") && !l[2].contains("--"), "plain words: {l:?}");
        let d = std::env::temp_dir().join(format!("den-lite-mods-{}", std::process::id()));
        std::fs::create_dir_all(d.join("Leyline")).unwrap();
        std::fs::write(d.join("Leyline/mods.json"), r#"{"schema":1,"build":"12340","mods":[{"key":"leyline:addons","name":"Leyline addons","version":"1.4","enabled":true,"addons":["LeylineDen"],"patches":[]},{"key":"mod:hd","name":"HD water","version":"","enabled":false,"patches":["Data/patch-4.MPQ"]}],"created_dirs":[]}"#).unwrap();
        let l = mods_lines(&names, Some(&d));
        assert_eq!(l[2], "Leyline's mods in this game: Leyline addons 1.4, HD water (off) (game files: restart to change).");
        let _ = std::fs::remove_dir_all(&d);
        assert!(addon_names(&[1, 2]).is_empty());
    }
}
