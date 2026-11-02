//! The in-game Den panel and Builder (WrathUnleashedUI EditMode.lua, Builder.lua, DenHome.lua) against Den Lite.
//!
//! The addon talks over the realm's OVMOD channel (realm/custom/lua_scripts/ov_modapi.lua): an addon whisper to
//! yourself, `OVMOD` `\t` `<mod>\t<name>\t<i>/<n>\t<data>`. On the full Den the realm's `ov_den.lua` (mod `wuden`)
//! and `ov_build.lua` (mod `wubuild`) answer; here this file does, for what Den Lite can do:
//!
//! * `wuden info` -> `role=..;theme=..;builders=-;invite=1`; `wuden cmd <text>` -> the `.den <text>` chat command,
//!   plus `invite` and `forge` (asked of the launcher: [`ASK_FILE`] / [`ANSWER_FILE`]);
//! * `wubuild catalog` (the props Den Lite knows, in the Builder's categories), `wubuild state` (the selection) and
//!   `wubuild cmd`: `spawn <entry>`, `near`, `move x|y|z <d>`, `move <id> x y z yaw`, `setpos`, `turn`, `setturn`,
//!   `tilt pitch|roll`, `scale <id> <s>`, `setscale`, `delete`, `undo`, `selclear`, `save`, `load`, `help`;
//!   `snap on|off` (edit mode's gate: the host yes, guests no unless `.den friends build on`).
//!
//! The 3.3.5a wire format of an addon message (the message crate has no `LANG_ADDON`, so it is read and written
//! here): CMSG_MESSAGECHAT `u32 type, u32 language (0xFFFFFFFF), [cstring target for a whisper], cstring text`;
//! SMSG_MESSAGECHAT `u8 type, u32 language, u64 sender, u32 flags, u64 target, u32 len, text\0, u8 tag`.

use crate::den::{self, Change, Den, MAX_PROPS};
use std::path::Path;

pub const CMSG_MESSAGECHAT: u32 = 0x0095;
pub const SMSG_MESSAGECHAT: u16 = 0x0096;
pub const LANG_ADDON: u32 = 0xFFFF_FFFF;
pub const CHAT_WHISPER: u32 = 7;
pub const PREFIX: &str = "OVMOD";
/// `<dens>/den-lite.ask`: what the game asked the launcher for: `{"id": n, "ask": "invite"|"forge"}`.
pub const ASK_FILE: &str = "den-lite.ask";
/// `<dens>/den-lite.answer`: the launcher's answer: `{"id": n, "invite": "<link>"}` or `{"id": n, "say": "..."}`.
pub const ANSWER_FILE: &str = "den-lite.answer";
/// One part's data, so `OVMOD\t<mod>\t<name>\t<i>/<n>\t<data>` stays under the client's 255.
const PART: usize = 200;
const MAX_PARTS: usize = 32;

/// An addon message from the client: its prefix and the text after the tab.
#[derive(Debug, Clone, PartialEq)]
pub struct AddonMsg {
    pub prefix: String,
    pub text: String,
}

fn cstr(b: &[u8]) -> Option<(&[u8], &[u8])> {
    let i = b.iter().position(|&c| c == 0)?;
    Some((&b[..i], &b[i + 1..]))
}

/// The client's CMSG_MESSAGECHAT body, when it is an addon message (whispered, or to the party, raid or guild).
pub fn parse_addon(body: &[u8]) -> Option<AddonMsg> {
    if body.len() < 8 {
        return None;
    }
    let kind = u32::from_le_bytes(body[0..4].try_into().ok()?);
    let lang = u32::from_le_bytes(body[4..8].try_into().ok()?);
    if lang != LANG_ADDON {
        return None;
    }
    let mut rest = &body[8..];
    if kind == CHAT_WHISPER {
        rest = cstr(rest)?.1;
    }
    let (msg, _) = cstr(rest).unwrap_or((rest, &[]));
    let msg = String::from_utf8_lossy(msg).to_string();
    let (prefix, text) = msg.split_once('\t')?;
    Some(AddonMsg { prefix: prefix.to_string(), text: text.to_string() })
}

/// `<mod>\t<name>\t<i>/<n>\t<data>`, read (only n == 1: what the addon sends).
pub fn split(text: &str) -> Option<(&str, &str, &str)> {
    let mut it = text.splitn(4, '\t');
    let (m, n, part, data) = (it.next()?, it.next()?, it.next()?, it.next().unwrap_or(""));
    (part == "1/1").then_some((m, n, data))
}

/// SMSG_MESSAGECHAT bodies for one answer (`OVMOD\t<mod>\t<name>\t<i>/<n>\t<part>`), whispered from `me` to `me`.
pub fn answer_bodies(me: u64, md: &str, name: &str, data: &str) -> Vec<Vec<u8>> {
    let mut parts: Vec<&str> = Vec::new();
    let mut rest = data;
    while !rest.is_empty() && parts.len() < MAX_PARTS {
        let mut cut = rest.len().min(PART);
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        parts.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    if parts.is_empty() {
        parts.push("");
    }
    let n = parts.len();
    parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = format!("{PREFIX}\t{md}\t{name}\t{}/{n}\t{p}", i + 1);
            let mut b = Vec::with_capacity(text.len() + 32);
            b.push(CHAT_WHISPER as u8);
            b.extend_from_slice(&LANG_ADDON.to_le_bytes());
            b.extend_from_slice(&me.to_le_bytes());
            b.extend_from_slice(&0u32.to_le_bytes());
            b.extend_from_slice(&me.to_le_bytes());
            b.extend_from_slice(&((text.len() + 1) as u32).to_le_bytes());
            b.extend_from_slice(text.as_bytes());
            b.push(0);
            b.push(0);
            b
        })
        .collect()
}

/// What the world server does with an addon message.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    /// An addon answer: mod, name, data.
    Addon(String, String, String),
    /// A system line in the chat.
    Say(String),
    /// A change to the Den, for everyone in it.
    Change(Change),
    /// Run as the chat command `.den <text>`.
    DenCmd(String),
    /// Ask the launcher (`invite`, `forge`).
    Ask(String),
    /// The first answer to the panel or the Builder (the `builder_ready` milestone).
    Ready,
}

/// Where the player stands and faces (world coordinates, radians).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Here {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub o: f32,
}

/// One session's Builder state.
#[derive(Debug, Default, Clone)]
pub struct Sel {
    pub guid: Option<u64>,
}

/// The Builder's categories (Builder.lua CATS) by words in a prop's name; None: not offered in the catalog.
pub fn category(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    let has = |ws: &[&str]| ws.iter().any(|w| n.contains(w));
    if has(&["marker", "trigger", "portal", "invisible", "collision", "[ph]", "test"]) {
        return None;
    }
    Some(if has(&["chair", "bench", "stool", "seat", "throne", "pew"]) {
        "seating"
    } else if has(&["table", "desk", "counter"]) {
        "tables"
    } else if has(&["barrel", "keg", "cask"]) {
        "barrels"
    } else if has(&["crate", "box", "chest"]) {
        "crates"
    } else if has(&["lantern", "lamp", "torch", "candle", "light", "chandelier", "sconce"]) {
        "lights"
    } else if has(&["banner", "flag"]) {
        "banners"
    } else if has(&["tent", "pavilion"]) {
        "tents"
    } else if has(&["fire", "brazier", "bonfire", "campfire", "hearth"]) {
        "fires"
    } else if has(&["fence", "gate", "wall", "post"]) {
        "fences"
    } else if has(&["tree", "bush", "flower", "plant", "rock", "stone", "grass", "shrub", "mushroom", "vine", "log"]) {
        "nature"
    } else if has(&["house", "inn", "tower", "hut", "barn", "chapel", "farm", "blacksmith", "mill", "stable", "cottage"]) {
        "buildings"
    } else if has(&["floor", "tile", "block", "pillar", "column", "stair", "beam", "plank"]) {
        "structure"
    } else {
        "decor"
    })
}

fn key(name: &str) -> String {
    let k: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).take(20).collect();
    if k.is_empty() { "prop".into() } else { k }
}

/// The catalog line: `id,key,category,name,points;...`, as many as fit the client's 32 parts.
pub fn catalog() -> String {
    let mut out = String::new();
    for o in crate::objects::OBJECTS {
        let Some(cat) = category(o.3) else { continue };
        let name: String = o.3.chars().filter(|c| !matches!(c, ';' | ',' | '|' | '\t')).collect();
        let item = format!("{},{},{},{},1;", o.0, key(&name), cat, name);
        if out.len() + item.len() > PART * MAX_PARTS {
            break;
        }
        out.push_str(&item);
    }
    out
}

/// A prop's id in the Builder: its object guid's counter (also its `gid`).
pub fn prop_id(guid: u64) -> u64 {
    guid & 0xFF_FFFF
}

fn guid_of(d: &Den, id: u64) -> Option<u64> {
    d.props.iter().map(|p| p.obj.guid).find(|g| prop_id(*g) == id)
}

/// The state line (Builder.lua parseState) for this session's selection.
pub fn state(d: &Den, sel: &Sel, role: &str) -> String {
    let n = d.props.len();
    let mut s = format!("mine={n}/{MAX_PROPS};pts={n}/{MAX_PROPS};undo={};redo=0;stored=0;grp=0;den={role};room={n}/{MAX_PROPS};yard=0/0;core=1;area=den;code=-;", d.undo_depth());
    match sel.guid.and_then(|g| d.placed(g)) {
        Some(p) => {
            let name = den::template(p.obj.entry).map(|t| t.name).unwrap_or("Prop").replace([';', ',', '|'], "");
            let id = prop_id(p.obj.guid);
            s.push_str(&format!(
                "sel={id},{},{name};n=1;pos={:.2},{:.2},{:.2},{:.0},{:.2};rot={:.0},{:.0},{:.0};ids={id};gids={id};",
                p.prop.entry,
                p.obj.x,
                p.obj.y,
                p.obj.z,
                p.prop.yaw,
                p.obj.scale,
                p.prop.yaw,
                p.prop.pitch.unwrap_or(0.0),
                p.prop.roll.unwrap_or(0.0)
            ));
        }
        None => s.push_str("sel=-;n=0;ids=;gids=;"),
    }
    s
}

fn num(s: Option<&&str>) -> Option<f64> {
    s.and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite())
}

const BUILDER_HELP: &str = "Den Lite's Builder: Spawn a prop where you stand, Near picks the closest, then move (x, y, z), turn, tilt, size, Delete and Undo. Your Den keeps itself; Save and Load in the Den panel (/den).";

/// One Builder command (`wubuild cmd <text>`) on the Den. `can` is false for a friend who may not build.
pub fn builder_cmd(d: &mut Den, sel: &mut Sel, here: Here, text: &str, can: bool) -> Vec<Out> {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let looks = matches!(verb.as_str(), "help" | "near" | "selclear" | "" | "catalog");
    if !can && !looks {
        return vec![Out::Say("Only the Den's owner builds here (they can let friends: .den friends build on).".into())];
    }
    // one prop's change; `f` gets the prop (relative to the Den) and the Den's spot (for world coordinates)
    let edit = |d: &mut Den, sel: &Sel, f: &dyn Fn(&mut den::Prop, (f64, f64, f64))| -> Out {
        let Some(g) = sel.guid else { return Out::Say("Select a prop first (Near picks the closest).".into()) };
        let spot = (d.spot.x as f64, d.spot.y as f64, d.spot.z as f64);
        match d.edit(g, |p| f(p, spot)) {
            Ok(c) => Out::Change(c),
            Err(e) => Out::Say(e),
        }
    };
    let out = match verb.as_str() {
        "spawn" => match w.get(1).and_then(|e| e.parse::<u32>().ok()) {
            Some(entry) => {
                let (x, y) = (here.x + 3.0 * here.o.cos(), here.y + 3.0 * here.o.sin());
                let z = d.floor_at(x, y);
                match d.spawn(entry, x, y, z, here.o) {
                    Ok(obj) => {
                        sel.guid = Some(obj.guid);
                        Out::Change(Change { created: vec![obj], message: "Placed. Move, turn or Delete it; Undo takes it back.".into(), ..Default::default() })
                    }
                    Err(e) => Out::Say(e),
                }
            }
            None => Out::Say("Pick a prop in the list first.".into()),
        },
        "near" => match d.nearest(here.x, here.y, here.z, 12.0) {
            Some(g) => {
                sel.guid = Some(g);
                let name = d.placed(g).and_then(|p| den::template(p.obj.entry)).map(|t| t.name).unwrap_or("a prop");
                Out::Say(format!("Selected {name}."))
            }
            None => Out::Say("No prop within 12 yards.".into()),
        },
        "selclear" => {
            sel.guid = None;
            Out::Say("Selection cleared.".into())
        }
        "move" if w.len() >= 6 => {
            // move <id> x y z yaw [pitch roll]: exact numbers, world coordinates
            let id = w[1].parse::<u64>().ok();
            if let Some(g) = id.and_then(|i| guid_of(d, i)) {
                sel.guid = Some(g);
            }
            let (x, y, z, yaw) = (num(w.get(2)), num(w.get(3)), num(w.get(4)), num(w.get(5)));
            let (pitch, roll) = (num(w.get(6)), num(w.get(7)));
            match (x, y, z, yaw) {
                (Some(x), Some(y), Some(z), Some(yaw)) => edit(d, sel, &move |p, s| {
                    p.x = x - s.0;
                    p.y = y - s.1;
                    p.z = z - s.2;
                    p.yaw = yaw;
                    if pitch.is_some() {
                        p.pitch = pitch;
                    }
                    if roll.is_some() {
                        p.roll = roll;
                    }
                }),
                _ => Out::Say("Usage: move <id> x y z yaw".into()),
            }
        }
        "move" => match (w.get(1).map(|a| a.to_ascii_lowercase()), num(w.get(2))) {
            (Some(a), Some(v)) if matches!(a.as_str(), "x" | "y" | "z") && v.abs() <= 50.0 => edit(d, sel, &move |p, _| match a.as_str() {
                "x" => p.x += v,
                "y" => p.y += v,
                _ => p.z += v,
            }),
            _ => Out::Say("Usage: move x|y|z <yards>".into()),
        },
        "setpos" => match (num(w.get(1)), num(w.get(2)), num(w.get(3))) {
            (Some(x), Some(y), Some(z)) => edit(d, sel, &move |p, s| {
                p.x = x - s.0;
                p.y = y - s.1;
                p.z = z - s.2;
            }),
            _ => Out::Say("Usage: setpos x y z".into()),
        },
        "turn" => match num(w.get(1)) {
            Some(a) => edit(d, sel, &move |p, _| p.yaw += a),
            None => Out::Say("Usage: turn <degrees>".into()),
        },
        "setturn" => match num(w.get(1)) {
            Some(a) => edit(d, sel, &move |p, _| p.yaw = a),
            None => Out::Say("Usage: setturn <degrees>".into()),
        },
        "tilt" => match (w.get(1).map(|a| a.to_ascii_lowercase()), num(w.get(2))) {
            (Some(a), Some(v)) if a == "pitch" || a == "roll" => edit(d, sel, &move |p, _| {
                let f = if a == "pitch" { &mut p.pitch } else { &mut p.roll };
                *f = Some((f.unwrap_or(0.0) + v).clamp(-180.0, 180.0));
            }),
            _ => Out::Say("Usage: tilt pitch|roll <degrees>".into()),
        },
        "scale" | "setscale" => {
            let (id, v) = if verb == "scale" { (w.get(1).and_then(|i| i.parse::<u64>().ok()), num(w.get(2))) } else { (None, num(w.get(1))) };
            if let Some(g) = id.and_then(|i| guid_of(d, i)) {
                sel.guid = Some(g);
            }
            match v {
                Some(v) if (0.1..=5.0).contains(&v) => edit(d, sel, &move |p, _| p.scale = Some(v)),
                _ => Out::Say("Size: 0.1 to 5.".into()),
            }
        }
        "delete" => match sel.guid.map(|g| d.delete(g)) {
            Some(Ok(c)) => {
                sel.guid = None;
                Out::Change(c)
            }
            Some(Err(e)) => Out::Say(e),
            None => Out::Say("Select a prop first (Near picks the closest).".into()),
        },
        "undo" => {
            let c = d.undo();
            if sel.guid.is_some_and(|g| d.placed(g).is_none()) {
                sel.guid = None;
            }
            Out::Change(c)
        }
        "save" | "load" => Out::DenCmd(w.join(" ")),
        "help" | "" => Out::Say(BUILDER_HELP.into()),
        _ => Out::Say(format!("Den Lite's Builder doesn't do \"{verb}\" yet. {BUILDER_HELP}")),
    };
    vec![out]
}

/// One addon message from a session. `owner`: the host; `can_build`: may change the Den.
pub fn handle(d: &mut Den, sel: &mut Sel, here: Option<Here>, msg: &AddonMsg, owner: bool, can_build: bool) -> Vec<Out> {
    if msg.prefix != PREFIX {
        return Vec::new();
    }
    let Some((md, name, data)) = split(&msg.text) else { return Vec::new() };
    let role = if owner { "owner" } else if can_build { "builder" } else { "visitor" };
    match (md, name) {
        ("wuden", "info") => vec![Out::Addon("wuden".into(), "info".into(), format!("role={role};theme={};builders=-;invite={}", d.look.name, owner as u8)), Out::Ready],
        ("wuden", "cmd") => {
            let w = data.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            match w.as_str() {
                "invite" | "forge" if owner => vec![Out::Ask(w)],
                "invite" | "forge" => vec![Out::Say("Only the Den's owner can do that.".into())],
                "allow" => vec![Out::Say("In Den Lite every friend with an invite link joins; .den friends build on lets them build.".into())],
                "budget" => vec![Out::Say(format!("Your Den has {} of {MAX_PROPS} props.", d.props.len()))],
                "home" => vec![Out::Say("You are in your Den.".into())],
                _ => vec![Out::DenCmd(data.to_string())],
            }
        }
        ("wubuild", "catalog") => vec![Out::Addon("wubuild".into(), "catalog".into(), catalog()), Out::Ready],
        ("wubuild", "state") => vec![Out::Addon("wubuild".into(), "state".into(), state(d, sel, role))],
        ("wubuild", "cmd") => {
            let w = data.trim();
            if let Some(rest) = w.strip_prefix("snap") {
                let on = rest.trim() != "off";
                let answer = if !on { "off,done,".to_string() } else if can_build { "on".into() } else { "off,role,Only the Den's owner builds here.".into() };
                return vec![Out::Addon("wubuild".into(), "snap".into(), answer)];
            }
            let Some(h) = here else { return Vec::new() };
            let mut v = builder_cmd(d, sel, h, w, can_build);
            v.push(Out::Addon("wubuild".into(), "state".into(), state(d, sel, role)));
            v
        }
        _ => Vec::new(),
    }
}

/// Ask the launcher: `<dens>/den-lite.ask` (one at a time; a newer one replaces it). The id it got.
pub fn ask(dir: &Path, what: &str) -> u64 {
    let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let _ = std::fs::write(dir.join(ASK_FILE), serde_json::json!({ "id": id, "ask": what }).to_string());
    id
}

/// What the launcher answered, taken (the file is removed): an invite link (checked) or a line to say.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Invite(String),
    Say(String),
}

pub fn take_answer(dir: &Path) -> Option<Answer> {
    let p = dir.join(ANSWER_FILE);
    let t = std::fs::read_to_string(&p).ok()?;
    let _ = std::fs::remove_file(&p);
    let v: serde_json::Value = serde_json::from_str(&t).ok()?;
    if let Some(l) = v.get("invite").and_then(|l| l.as_str()) {
        let ok = l.len() <= 160 && l.starts_with("https://") && l.contains("/join/") && l.bytes().all(|b| b.is_ascii_graphic());
        return ok.then(|| Answer::Invite(l.to_string()));
    }
    v.get("say").and_then(|s| s.as_str()).map(|s| Answer::Say(s.chars().filter(|c| !c.is_control()).take(200).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn whisper(text: &str) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&CHAT_WHISPER.to_le_bytes());
        b.extend_from_slice(&LANG_ADDON.to_le_bytes());
        b.extend_from_slice(b"Wren\0");
        b.extend_from_slice(text.as_bytes());
        b.push(0);
        b
    }

    #[test]
    fn an_addon_whisper_is_read_and_answered_in_parts() {
        let m = parse_addon(&whisper("OVMOD\twuden\tinfo\t1/1\t")).unwrap();
        assert_eq!(m, AddonMsg { prefix: "OVMOD".into(), text: "wuden\tinfo\t1/1\t".into() });
        assert_eq!(split(&m.text), Some(("wuden", "info", "")));
        let mut say = whisper("hello");
        say[4..8].copy_from_slice(&7u32.to_le_bytes());
        assert!(parse_addon(&say).is_none(), "not an addon message");
        let b = answer_bodies(5, "wubuild", "catalog", &"x".repeat(450));
        assert_eq!(b.len(), 3);
        let text = |b: &[u8]| String::from_utf8_lossy(&b[29..b.len() - 2]).to_string();
        assert!(text(&b[0]).starts_with("OVMOD\twubuild\tcatalog\t1/3\t"));
        assert!(text(&b[2]).ends_with("3/3\txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
        assert!(b.iter().all(|x| x.len() - 31 < 255));
        assert_eq!(answer_bodies(5, "wuden", "info", "").len(), 1);
    }

    #[test]
    fn the_catalog_fits_and_has_the_categories() {
        let c = catalog();
        assert!(c.len() <= PART * MAX_PARTS);
        assert!(c.contains("2413,woodenchair,seating,Wooden Chair,1;"), "{}", &c[..200]);
        assert!(c.split(';').filter(|s| !s.is_empty()).all(|s| s.split(',').count() == 5));
    }

    #[test]
    fn spawn_select_move_turn_delete_and_undo() {
        let mut d = Den::new(den::Spot::default());
        let s0 = den::Spot::default();
        let here = Here { x: s0.x, y: s0.y, z: s0.z, o: 0.0 };
        let mut sel = Sel::default();
        let out = builder_cmd(&mut d, &mut sel, here, "spawn 2413", true);
        let Out::Change(c) = &out[0] else { panic!("{out:?}") };
        assert_eq!(c.created.len(), 1);
        let g = sel.guid.unwrap();
        assert!((d.placed(g).unwrap().obj.x - (s0.x + 3.0)).abs() < 0.01, "3 yards ahead");
        let st = state(&d, &sel, "owner");
        assert!(st.contains(&format!("sel={},2413,Wooden Chair", prop_id(g))) && st.contains("undo=1"), "{st}");
        builder_cmd(&mut d, &mut sel, here, "move z 0.5", true);
        assert!((d.placed(g).unwrap().prop.z - 0.5).abs() < 1e-6);
        builder_cmd(&mut d, &mut sel, here, "turn 45", true);
        assert_eq!(d.placed(g).unwrap().prop.yaw, 45.0);
        let o = builder_cmd(&mut d, &mut sel, here, "move x 99", true);
        assert!(matches!(&o[0], Out::Say(_)), "too far in one step");
        let o = builder_cmd(&mut d, &mut sel, here, "move y 30", true);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("outside")), "{o:?}");
        let id = prop_id(g);
        builder_cmd(&mut d, &mut sel, here, &format!("move {id} {} {} {} 90", s0.x + 1.0, s0.y + 1.0, s0.z), true);
        let p = &d.placed(g).unwrap().prop;
        assert_eq!((p.x, p.y, p.yaw), (1.0, 1.0, 90.0));
        builder_cmd(&mut d, &mut sel, here, &format!("scale {id} 2"), true);
        assert_eq!(d.placed(g).unwrap().obj.scale, 2.0);
        // undo walks back: size, exact move, turn, rise, spawn
        for _ in 0..4 {
            builder_cmd(&mut d, &mut sel, here, "undo", true);
        }
        assert_eq!(d.placed(g).unwrap().prop.z, 0.0);
        builder_cmd(&mut d, &mut sel, here, "delete", true);
        assert!(d.props.is_empty() && sel.guid.is_none());
        builder_cmd(&mut d, &mut sel, here, "undo", true);
        assert_eq!(d.props.len(), 1, "delete undone");
        builder_cmd(&mut d, &mut sel, here, "near", true);
        assert!(sel.guid.is_some());
        let o = builder_cmd(&mut d, &mut sel, here, "delete", false);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("owner")), "a visitor can't");
        assert_eq!(d.props.len(), 1);
    }

    #[test]
    fn the_den_panel_and_the_builders_gate() {
        let mut d = Den::new(den::Spot::default());
        let mut sel = Sel::default();
        let m = |t: &str| AddonMsg { prefix: "OVMOD".into(), text: t.into() };
        let o = handle(&mut d, &mut sel, None, &m("wuden\tinfo\t1/1\t"), true, true);
        assert!(matches!(&o[0], Out::Addon(a, b, c) if a == "wuden" && b == "info" && c.starts_with("role=owner;theme=white")));
        assert!(o.contains(&Out::Ready));
        assert_eq!(handle(&mut d, &mut sel, None, &m("wuden\tcmd\t1/1\tinvite"), true, true), vec![Out::Ask("invite".into())]);
        assert_eq!(handle(&mut d, &mut sel, None, &m("wuden\tcmd\t1/1\tsave mine"), true, true), vec![Out::DenCmd("save mine".into())]);
        assert!(matches!(&handle(&mut d, &mut sel, None, &m("wuden\tcmd\t1/1\tforge"), false, false)[0], Out::Say(_)));
        assert_eq!(handle(&mut d, &mut sel, None, &m("wubuild\tcmd\t1/1\tsnap on"), true, true), vec![Out::Addon("wubuild".into(), "snap".into(), "on".into())]);
        assert!(matches!(&handle(&mut d, &mut sel, None, &m("wubuild\tcmd\t1/1\tsnap on"), false, false)[0], Out::Addon(_, _, a) if a.starts_with("off,role")));
        assert!(handle(&mut d, &mut sel, None, &AddonMsg { prefix: "Other".into(), text: "x".into() }, true, true).is_empty());
    }

    #[test]
    fn the_launchers_answer_is_checked() {
        let d = std::env::temp_dir().join(format!("den-ask-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let id = ask(&d, "invite");
        assert!(std::fs::read_to_string(d.join(ASK_FILE)).unwrap().contains(&id.to_string()));
        std::fs::write(d.join(ANSWER_FILE), r#"{"id":1,"invite":"https://relay.openvanilla.app/join/abc#DEF~~GAB.CD"}"#).unwrap();
        assert_eq!(take_answer(&d), Some(Answer::Invite("https://relay.openvanilla.app/join/abc#DEF~~GAB.CD".into())));
        assert!(!d.join(ANSWER_FILE).exists());
        std::fs::write(d.join(ANSWER_FILE), r#"{"id":1,"invite":"javascript:x"}"#).unwrap();
        assert_eq!(take_answer(&d), None);
        std::fs::write(d.join(ANSWER_FILE), r#"{"id":1,"say":"Forge is opening."}"#).unwrap();
        assert_eq!(take_answer(&d), Some(Answer::Say("Forge is opening.".into())));
        let _ = std::fs::remove_dir_all(&d);
    }
}
