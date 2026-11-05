//! The in-game Den panel and Builder (WrathUnleashedUI EditMode.lua, Builder.lua, DenHome.lua) against Den Lite.
//!
//! The addon talks over the realm's OVMOD channel (realm/custom/lua_scripts/ov_modapi.lua): an addon whisper to
//! yourself, `OVMOD` `\t` `<mod>\t<name>\t<i>/<n>\t<data>`. On the full Den the realm's `ov_den.lua` (mod `wuden`)
//! and `ov_build.lua` (mod `wubuild`) answer; here this file does, for what Den Lite can do:
//!
//! * `wuden hello <version>` (the Den bar, DenBar.lua, notice.rs): from now on what Den Lite says that nobody typed
//!   for is a toast in the bar (`wuden toast <kind>|<text>`), and the bar is told `wuden status`
//!   (`role=..;props=..;saved=<seconds ago>|-`), and `wuden saved auto|<name>` after each save; `wuden status` asks
//!   again;
//! * `wuden info` -> `role=..;theme=..;builders=-;invite=1`; `wuden cmd <text>` -> the `.den <text>` chat command,
//!   plus `invite` and `forge` (asked of the launcher: [`ASK_FILE`] / [`ANSWER_FILE`]);
//! * `wubuild catalog` (the props Den Lite knows, in the Builder's categories, with each model's path when the
//!   player's own GameObjectDisplayInfo is in the Dens folder: `models.rs`; what doesn't fit one message follows as
//!   `wubuild catmore`), `wubuild state` (the selection) and
//!   `wubuild cmd`: `spawn <entry>`, `near`, `move x|y|z <d>`, `move <id> x y z yaw`, `setpos`, `turn`, `setturn`,
//!   `tilt pitch|roll`, `scale <id> <s>`, `setscale`, `delete`, `undo`, `redo`, `copy`, `paste`, `dup`, `selall`
//!   (your last prop: Den Lite selects one at a time), `pick <id>`, `floor`, `grid <yd>|off`, `spawnat <entry> x y z`,
//!   `find <word>` / `models <word>` (answered `wubuild found`), `selclear`, `save`, `load`, `help`;
//!   `selnext`, `selkind`, `lock`, `unlock` (the selection panel), the placement aids' `preview`, `hover`, `follow`,
//!   `commit`, `axis`, `gizmo`, `mode`, `where` (gizmo.rs, handled in world.rs before this file sees them);
//!   `snap on|off` (edit mode's gate: the host yes, guests no unless `.den friends build on`). The state says `lite=1`:
//!   the editor then hides what only the full Den does (groups, storage, prefabs, plots, align, stack, hide); `caps=` lists
//!   every verb answered here.
//! * `wuden report <text>` (the addon's own check, asked by the den-test's `.den ui report`, world.rs): written to the
//!   events (`ui_report`) and `<dens>/den-lite.ui-report` while the test's channel is on.
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
    /// The Den bar is here (notice.rs): what waited for it, and the status line.
    Hello,
    /// The status line again.
    Status,
    /// The addon's own report (the den-test's feature check): its text.
    Report(String),
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
    /// Copy's prop (relative to the Den, its turn, tilt and size) for Paste.
    pub clip: Option<den::Prop>,
    /// Snap: what Spawn and Paste put down lands on this grid (yards; 0 off).
    pub grid: f64,
    /// `selkind`: the other props selected with `guid` (every one with its entry); Delete and the moves and turns
    /// take them all. Any other change of the selection empties it.
    pub more: Vec<u64>,
    /// `selnext`: where the round started (the props in order of their distance from here, wrapping).
    pub round: Option<[f32; 3]>,
}

/// The verbs this Den Lite answers (the state's `caps=`: the editor hides a button whose verb isn't here).
pub const CAPS: &str = "spawn,near,move,setpos,turn,setturn,tilt,scale,setscale,delete,undo,redo,copy,paste,dup,selall,pick,selclear,floor,grid,spawnat,find,models,save,load,selnext,selkind,seladd,lock,unlock,swap,swapkind,bring,ground,settle,upright,random,centre,reset,step,grow,preview,hover,follow,commit,axis,grab,cam,gizmo,mode,where,click,drag";

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

/// The whole catalog: `id,key,category,name,points[,model];...` in messages that each fit the client's 32 parts: the
/// first is `catalog`, the rest `catmore` (Builder.lua adds them to the first; an older addon keeps the first). The
/// model (the gallery's 3D tile) is the path in the player's own GameObjectDisplayInfo ([`crate::models`]), when the
/// launcher put that table in the Dens folder; without it the lines have five fields, as before.
pub fn catalog_parts(model_of: impl Fn(u32) -> Option<String>) -> Vec<String> {
    let mut parts = vec![String::new()];
    // the player's own models (mymodels.rs) in their own category, then every prop Den Lite knows
    let mine: Vec<(u32, u8, u32, &'static str, f32)> = crate::mymodels::all().into_iter().map(|t| (t.entry, t.kind, t.display, t.name, t.size)).collect();
    for o in mine.iter().chain(crate::objects::OBJECTS.iter()) {
        let cat = if crate::mymodels::is_mine(o.0) { Some(crate::mymodels::CATEGORY) } else { category(o.3) };
        let Some(cat) = cat else { continue };
        let name: String = o.3.chars().filter(|c| !matches!(c, ';' | ',' | '|' | '\t')).collect();
        let model = model_of(o.2).map(|m| format!(",{m}")).unwrap_or_default();
        let item = format!("{},{},{},{},1{};", o.0, key(&name), cat, name, model);
        if parts.last().is_some_and(|p| p.len() + item.len() > PART * MAX_PARTS) {
            parts.push(String::new());
        }
        if let Some(p) = parts.last_mut() {
            p.push_str(&item);
        }
    }
    parts
}

/// The catalog's first message (see [`catalog_parts`]).
pub fn catalog() -> String {
    catalog_parts(crate::models::tile).into_iter().next().unwrap_or_default()
}

/// `wubuild found`: a search's answer (Builder.lua: `<kind>,<page>,<more>;` then catalog items), the Den's own props
/// whose names have the word.
pub fn found(kind: &str, q: &str) -> String {
    let mut out = format!("{kind},1,0;");
    for o in den::find_objects(q).into_iter().take(40) {
        let name: String = o.3.chars().filter(|c| !matches!(c, ';' | ',' | '|' | '\t')).collect();
        let model = crate::models::tile(o.2).map(|m| format!(",{m}")).unwrap_or_default();
        out.push_str(&format!("{},{},{},{},1{};", o.0, key(&name), if crate::mymodels::is_mine(o.0) { crate::mymodels::CATEGORY } else { category(o.3).unwrap_or("decor") }, name, model));
    }
    out
}

/// The catalog's messages: `catalog`, then `catmore` for the rest.
fn catalog_out() -> Vec<Out> {
    let mut v: Vec<Out> = catalog_parts(crate::models::tile)
        .into_iter()
        .enumerate()
        .map(|(i, p)| Out::Addon("wubuild".into(), if i == 0 { "catalog" } else { "catmore" }.into(), p))
        .collect();
    v.push(Out::Ready);
    v
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
    let mut s = format!(
        "mine={n}/{MAX_PROPS};pts={n}/{MAX_PROPS};undo={};redo={};stored=0;grp=0;den={role};room={n}/{MAX_PROPS};yard=0/0;core=1;lite=1;grid={};clip={};area=den;code=-;caps={CAPS},{};",
        d.undo_depth(),
        d.redo_depth(),
        sel.grid,
        sel.clip.is_some() as u8,
        crate::buildtools::CAPS
    );
    match sel.guid.and_then(|g| d.placed(g)) {
        Some(p) => {
            let name = den::template(p.obj.entry).map(|t| t.name).unwrap_or("Prop").replace([';', ',', '|'], "");
            let id = prop_id(p.obj.guid);
            let ids: Vec<String> = std::iter::once(id).chain(sel.more.iter().filter(|g| d.placed(**g).is_some()).map(|g| prop_id(*g))).map(|i| i.to_string()).collect();
            let ids = ids.join(",");
            s.push_str(&format!(
                "sel={id},{},{name};n={};pos={:.2},{:.2},{:.2},{:.0},{:.2};rot={:.0},{:.0},{:.0};ids={ids};gids={ids};locked={};",
                p.prop.entry,
                1 + sel.more.iter().filter(|g| d.placed(**g).is_some()).count(),
                p.obj.x,
                p.obj.y,
                p.obj.z,
                p.prop.yaw,
                p.obj.scale,
                p.prop.yaw,
                p.prop.pitch.unwrap_or(0.0),
                p.prop.roll.unwrap_or(0.0),
                (p.prop.locked == Some(true)) as u8
            ));
            // the Tools window's: Solid, what the game does with its model, the group (buildtools.rs)
            s.push_str(&crate::buildtools::state_extra(d, sel));
        }
        None => s.push_str("sel=-;n=0;ids=;gids=;"),
    }
    s
}

fn num(s: Option<&&str>) -> Option<f64> {
    s.and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite())
}

const BUILDER_HELP: &str = "Den Lite's Builder: place a prop from the gallery, Near picks the closest, then move (x, y, z), turn, tilt, size, Copy, Paste, Duplicate, Delete, Undo and Redo. Your Den keeps itself; Files saves it by name.";

/// What only the full Den does: Den Lite says so plainly (the editor hides these on Den Lite: the state's `lite=1`).
/// (group, hide, show, measure, align and path are Den Lite's too since 4 Nov: buildtools.rs, which sees them first.)
const FULL_ONLY: &[&str] = &["store", "stack", "prefab", "plot", "share", "selbox", "stackon", "pasteat", "placeat"];

/// The pad's step (`step <dir> <yards>`) as (dx, dy, dz) for someone facing `o` (radians): forward is the way they
/// face, left a quarter turn to their left.
pub fn step_offset(dir: &str, o: f32, v: f64) -> Option<(f64, f64, f64)> {
    let (s, c) = (o as f64).sin_cos();
    Some(match dir {
        "fwd" | "forward" => (c * v, s * v, 0.0),
        "back" => (-c * v, -s * v, 0.0),
        "left" => (-s * v, c * v, 0.0),
        "right" => (s * v, -c * v, 0.0),
        "up" => (0.0, 0.0, v),
        "down" => (0.0, 0.0, -v),
        _ => return None,
    })
}

/// A turn for the Random button (degrees, 0 to 360): the clock's nanoseconds, spread.
fn random_degrees() -> f64 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as u64 ^ d.as_secs()).unwrap_or(7);
    (n.wrapping_mul(2_654_435_761) % 360_000) as f64 / 1000.0
}

/// (x, y) on the grid around the Den's middle (yards; 0: as it is).
fn snap(g: f64, v: f32, mid: f32) -> f32 {
    if g > 0.0 { mid + (((v - mid) as f64 / g).round() * g) as f32 } else { v }
}

/// One Builder command (`wubuild cmd <text>`) on the Den. `can` is false for a friend who may not build.
pub fn builder_cmd(d: &mut Den, sel: &mut Sel, here: Here, text: &str, can: bool) -> Vec<Out> {
    let verb = text.split_whitespace().next().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let was = sel.guid;
    let out = match verb.as_str() {
        "selnext" | "selkind" | "lock" | "unlock" => selection_cmd(d, sel, here, &verb, can),
        // Shift+right-click (Gizmo.lua): the prop picked before stays selected with the new one
        "seladd" => match text.split_whitespace().nth(1).and_then(|i| i.parse::<u64>().ok()).and_then(|i| guid_of(d, i)) {
            Some(g) if sel.guid.is_some() && sel.guid != Some(g) => {
                if !sel.more.contains(&g) {
                    sel.more.push(g);
                }
                vec![Out::Say(format!("{} selected.", 1 + sel.more.len()))]
            }
            Some(g) => {
                sel.guid = Some(g);
                vec![Out::Say("1 selected.".into())]
            }
            None => vec![Out::Say("That prop isn't in your Den.".into())],
        },
        // the props selected with `selkind` too: Delete and the moves and turns take them all
        "delete" | "move" | "turn" | "tilt" | "floor" | "ground" | "settle" | "upright" | "random" | "reset" | "step" | "grow" if !sel.more.is_empty() && !(verb == "move" && text.split_whitespace().count() >= 6) => {
            let mut v = builder_one(d, sel, here, text, can);
            let more = std::mem::take(&mut sel.more);
            let keep = sel.guid;
            for g in &more {
                if d.placed(*g).is_none() {
                    continue;
                }
                sel.guid = Some(*g);
                v.extend(builder_one(d, sel, here, text, can).into_iter().filter(|o| matches!(o, Out::Change(_))));
            }
            sel.guid = keep;
            if verb != "delete" {
                sel.more = more.into_iter().filter(|g| d.placed(*g).is_some()).collect();
            }
            v
        }
        _ => builder_one(d, sel, here, text, can),
    };
    // a selection changed any other way: the extra ones and the round go
    if sel.guid != was && verb != "selnext" && verb != "selkind" && verb != "seladd" {
        sel.more.clear();
        sel.round = None;
    }
    if sel.guid.is_none() {
        sel.more.clear();
    }
    out
}

/// `selnext`, `selkind`, `lock`, `unlock`.
fn selection_cmd(d: &mut Den, sel: &mut Sel, here: Here, verb: &str, can: bool) -> Vec<Out> {
    let name = |d: &Den, g: u64| d.placed(g).and_then(|p| den::template(p.obj.entry)).map(|t| t.name).unwrap_or("a prop");
    match verb {
        "selnext" => {
            if d.props.is_empty() {
                return vec![Out::Say("Nothing placed yet.".into())];
            }
            // the round: the props by their distance from where it started (the selection then, else you)
            let at = sel.round.unwrap_or_else(|| sel.guid.and_then(|g| d.placed(g)).map(|p| [p.obj.x, p.obj.y, p.obj.z]).unwrap_or([here.x, here.y, here.z]));
            sel.round = Some(at);
            let dist = |p: &den::Placed| (p.obj.x - at[0]).powi(2) + (p.obj.y - at[1]).powi(2) + (p.obj.z - at[2]).powi(2);
            let mut order: Vec<(f32, u64)> = d.props.iter().map(|p| (dist(p), p.obj.guid)).collect();
            order.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
            let i = sel.guid.and_then(|g| order.iter().position(|o| o.1 == g)).map(|i| (i + 1) % order.len()).unwrap_or(0);
            let g = order[i].1;
            sel.guid = Some(g);
            sel.more.clear();
            vec![Out::Say(format!("Selected {} ({} of {}).", name(&*d, g), i + 1, order.len()))]
        }
        "selkind" => match sel.guid.and_then(|g| d.placed(g)).map(|p| p.obj.entry) {
            Some(entry) => {
                let g0 = sel.guid;
                sel.more = d.props.iter().filter(|p| p.obj.entry == entry && Some(p.obj.guid) != g0).map(|p| p.obj.guid).collect();
                vec![Out::Say(format!("Selected every {}: {} props.", name(&*d, g0.unwrap_or(0)), sel.more.len() + 1))]
            }
            None => vec![Out::Say("Select a prop first (Near picks the closest).".into())],
        },
        _ => {
            if !can {
                return vec![Out::Say("Only the Den's owner builds here (they can let friends: .den friends build on).".into())];
            }
            let on = verb == "lock";
            let Some(g) = sel.guid else { return vec![Out::Say("Select a prop first (Near picks the closest).".into())] };
            let all: Vec<u64> = std::iter::once(g).chain(sel.more.iter().copied()).collect();
            let n = all.iter().filter(|g| d.set_lock(**g, on).is_ok()).count();
            vec![Out::Say(if on { format!("Locked {n}: it can't be moved or deleted until you Unlock it.") } else { format!("Unlocked {n}.") })]
        }
    }
}

/// One Builder command on the one selected prop.
fn builder_one(d: &mut Den, sel: &mut Sel, here: Here, text: &str, can: bool) -> Vec<Out> {
    let w: Vec<&str> = text.split_whitespace().collect();
    let verb = w.first().map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    let looks = matches!(verb.as_str(), "help" | "near" | "selclear" | "" | "catalog" | "copy" | "find" | "models" | "pick" | "select" | "selall" | "grid" | "selnext" | "selkind");
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
                let (x, y) = (snap(sel.grid, x, d.spot.x), snap(sel.grid, y, d.spot.y));
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
        // one prop at a time on Den Lite: "select mine" is your last one
        "selall" => match d.props.last().map(|p| p.obj.guid) {
            Some(g) => {
                sel.guid = Some(g);
                let name = d.placed(g).and_then(|p| den::template(p.obj.entry)).map(|t| t.name).unwrap_or("a prop");
                Out::Say(format!("Selected {name}, your last prop (Den Lite selects one at a time)."))
            }
            None => Out::Say("Nothing placed yet.".into()),
        },
        "pick" | "select" => match w.get(1).and_then(|i| i.parse::<u64>().ok()).and_then(|i| guid_of(d, i)) {
            Some(g) => {
                sel.guid = Some(g);
                let name = d.placed(g).and_then(|p| den::template(p.obj.entry)).map(|t| t.name).unwrap_or("a prop");
                Out::Say(format!("Selected {name}."))
            }
            None => Out::Say("That prop isn't in your Den.".into()),
        },
        "copy" => match sel.guid.and_then(|g| d.placed(g)) {
            Some(p) => {
                sel.clip = Some(p.prop.clone());
                let name = den::template(p.obj.entry).map(|t| t.name).unwrap_or("the prop");
                Out::Say(format!("Copied {name}. Paste puts one in front of you."))
            }
            None => Out::Say("Select a prop first (Near picks the closest).".into()),
        },
        "paste" | "dup" => {
            let src = if verb == "dup" { sel.guid.and_then(|g| d.placed(g)).map(|p| p.prop.clone()) } else { sel.clip.clone() };
            match src {
                Some(mut p) => {
                    if verb == "dup" {
                        // next to the original, a yard along x
                        p.x += 1.0;
                    } else {
                        let (x, y) = (here.x + 3.0 * here.o.cos(), here.y + 3.0 * here.o.sin());
                        let (x, y) = (snap(sel.grid, x, d.spot.x), snap(sel.grid, y, d.spot.y));
                        // as high above the ground here as it was above the ground where it was copied
                        let lift = p.z - (d.floor_at(d.spot.x + p.x as f32, d.spot.y + p.y as f32) - d.spot.z) as f64;
                        p.z = (d.floor_at(x, y) - d.spot.z) as f64 + lift;
                        p.x = (x - d.spot.x) as f64;
                        p.y = (y - d.spot.y) as f64;
                    }
                    match d.place_one(p) {
                        Ok(obj) => {
                            sel.guid = Some(obj.guid);
                            let msg = if verb == "dup" { "Duplicated next to it." } else { "Pasted in front of you." };
                            Out::Change(Change { created: vec![obj], message: msg.into(), ..Default::default() })
                        }
                        Err(e) => Out::Say(e),
                    }
                }
                None if verb == "dup" => Out::Say("Select a prop first (Near picks the closest).".into()),
                None => Out::Say("Copy a prop first.".into()),
            }
        }
        "floor" => match sel.guid.and_then(|g| d.placed(g)).map(|p| (p.obj.x, p.obj.y)) {
            Some((x, y)) => {
                let to = (d.floor_at(x, y) - d.spot.z) as f64;
                edit(d, sel, &move |p, _| p.z = to)
            }
            None => Out::Say("Select a prop first (Near picks the closest).".into()),
        },
        // the selection panel's quick buttons and keys (Builder.lua; owner 3 Nov: "it should function like this")
        "ground" | "settle" | "upright" => match sel.guid.and_then(|g| d.placed(g)).map(|p| (p.obj.x, p.obj.y)) {
            Some((x, y)) => {
                let to = (d.floor_at(x, y) - d.spot.z) as f64;
                let (down, straight) = (verb != "upright", verb != "ground");
                edit(d, sel, &move |p, _| {
                    if down {
                        p.z = to;
                    }
                    if straight {
                        p.pitch = None;
                        p.roll = None;
                    }
                })
            }
            None => Out::Say("Select a prop first (Near picks the closest).".into()),
        },
        "random" => {
            let a = random_degrees();
            edit(d, sel, &move |p, _| p.yaw = a)
        }
        "reset" => edit(d, sel, &|p, _| {
            p.yaw = 0.0;
            p.pitch = None;
            p.roll = None;
            p.scale = None;
        }),
        "centre" | "center" => {
            let to = (d.floor_at(d.spot.x, d.spot.y) - d.spot.z) as f64;
            edit(d, sel, &move |p, _| {
                p.x = 0.0;
                p.y = 0.0;
                p.z = to;
            })
        }
        // Home: in front of you, as high over the ground there as it was over the ground where it stood
        "bring" => match sel.guid.and_then(|g| d.placed(g)).map(|p| p.obj.z - d.floor_at(p.obj.x, p.obj.y)) {
            Some(lift) => {
                let (x, y) = (here.x + 3.0 * here.o.cos(), here.y + 3.0 * here.o.sin());
                let (x, y) = (snap(sel.grid, x, d.spot.x), snap(sel.grid, y, d.spot.y));
                let (rx, ry, rz) = ((x - d.spot.x) as f64, (y - d.spot.y) as f64, (d.floor_at(x, y) + lift - d.spot.z) as f64);
                edit(d, sel, &move |p, _| {
                    p.x = rx;
                    p.y = ry;
                    p.z = rz;
                })
            }
            None => Out::Say("Select a prop first (Near picks the closest).".into()),
        },
        // the pad: from where you stand (forward is the way you face), up and down
        "step" => match (w.get(1).map(|a| a.to_ascii_lowercase()), num(w.get(2))) {
            (Some(dir), Some(v)) if v.abs() <= 50.0 => match step_offset(&dir, here.o, v) {
                Some((dx, dy, dz)) => edit(d, sel, &move |p, _| {
                    p.x += dx;
                    p.y += dy;
                    p.z += dz;
                }),
                None => Out::Say("Usage: step fwd|back|left|right|up|down <yards>".into()),
            },
            _ => Out::Say("Usage: step fwd|back|left|right|up|down <yards>".into()),
        },
        "grow" => match num(w.get(1)).filter(|k| (0.2..=5.0).contains(k)) {
            Some(k) => edit(d, sel, &move |p, _| p.scale = Some((p.scale.unwrap_or(1.0) * k).clamp(0.1, 5.0))),
            None => Out::Say("Usage: grow <factor> (0.8 smaller, 1.25 bigger)".into()),
        },
        // Swap with: the selection becomes another prop (its place, turn and size kept); swapkind: every prop of the
        // selection's kind, one undo step for them all
        "swap" | "swapkind" => match (w.get(1).and_then(|e| e.parse::<u32>().ok()).filter(|e| den::template(*e).is_some()), sel.guid.and_then(|g| d.placed(g)).map(|p| (p.obj.guid, p.prop.entry))) {
            (None, _) => Out::Say("Pick what to swap it with in the gallery first.".into()),
            (_, None) => Out::Say("Select a prop first (Near picks the closest).".into()),
            (Some(new), Some((g, old))) => {
                let guids: Vec<u64> = if verb == "swap" { vec![g] } else { d.props.iter().filter(|p| p.prop.entry == old).map(|p| p.obj.guid).collect() };
                let to = den::template(new).map(|t| t.name).unwrap_or("it");
                match d.edit_many(&guids, |p| {
                    p.entry = new as u64;
                    p.model = None;
                }) {
                    Ok(mut c) => {
                        let n = c.created.len();
                        c.message = if n == 1 { format!("Swapped for {to}. Undo takes it back.") } else { format!("Swapped {n} props for {to} (one undo).") };
                        Out::Change(c)
                    }
                    Err(e) => Out::Say(e),
                }
            }
        },
        "grid" => {
            let g = match w.get(1).map(|a| a.to_ascii_lowercase()) {
                Some(a) if a == "off" => Some(0.0),
                Some(a) => a.parse::<f64>().ok().filter(|g| g.is_finite() && (0.0..=10.0).contains(g)),
                None => None,
            };
            match g {
                Some(g) => {
                    sel.grid = g;
                    Out::Say(if g > 0.0 { format!("Snap: what you place lands on a {g} yard grid.") } else { "Snap off.".into() })
                }
                None => Out::Say("Usage: grid <yards> | grid off".into()),
            }
        }
        "spawnat" => match (w.get(1).and_then(|e| e.parse::<u32>().ok()), num(w.get(2)), num(w.get(3)), num(w.get(4))) {
            (Some(entry), Some(x), Some(y), Some(z)) => {
                let (x, y) = (snap(sel.grid, x as f32, d.spot.x), snap(sel.grid, y as f32, d.spot.y));
                match d.spawn(entry, x, y, z as f32, here.o) {
                    Ok(obj) => {
                        sel.guid = Some(obj.guid);
                        Out::Change(Change { created: vec![obj], message: "Placed.".into(), ..Default::default() })
                    }
                    Err(e) => Out::Say(e),
                }
            }
            _ => Out::Say("Usage: spawnat <prop> x y z".into()),
        },
        "find" | "models" => {
            let q = w[1..].iter().filter(|x| x.parse::<u32>().is_err()).cloned().collect::<Vec<_>>().join(" ");
            let kind = if verb == "models" { "models" } else { "objects" };
            return vec![Out::Addon("wubuild".into(), "found".into(), found(kind, &q))];
        }
        "redo" => {
            let c = d.redo();
            if sel.guid.is_some_and(|g| d.placed(g).is_none()) {
                sel.guid = None;
            }
            Out::Change(c)
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
        v if FULL_ONLY.contains(&v) => Out::Say(format!("Den Lite doesn't do \"{v}\" (it is the full Den's). {BUILDER_HELP}")),
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
        ("wuden", "hello") => vec![Out::Hello, Out::Ready],
        ("wuden", "status") => vec![Out::Status],
        ("wuden", "report") => vec![Out::Report(data.chars().filter(|c| !c.is_control()).take(1000).collect())],
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
        // as the full Den's ov_build.lua: the state comes with the catalog (the editor knows Undo n, Redo n and lite=1 at once)
        ("wubuild", "catalog") => {
            let mut v = catalog_out();
            let at = v.len().saturating_sub(1);
            v.insert(at, Out::Addon("wubuild".into(), "state".into(), state(d, sel, role)));
            v
        }
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
    fn the_whole_catalog_in_parts_with_models() {
        let plain = catalog_parts(|_| None);
        // the built-in props only (another test may have added the player's own models, mymodels.rs, meanwhile)
        let mine = format!(",{},", crate::mymodels::CATEGORY);
        let n: usize = plain.iter().map(|p| p.split(';').filter(|s| !s.is_empty() && !s.contains(&mine)).count()).sum();
        let offered = crate::objects::OBJECTS.iter().filter(|o| category(o.3).is_some()).count();
        assert_eq!(n, offered, "every prop, none cut off");
        assert!(plain.iter().all(|p| p.len() <= PART * MAX_PARTS && !p.is_empty()));
        assert!(plain.len() > 1, "more than one message's worth");
        let with = catalog_parts(|d| (d == 39).then(|| "World\\Generic\\Chair01.mdx".to_string()));
        let all = with.concat();
        assert!(all.contains("2413,woodenchair,seating,Wooden Chair,1,World\\Generic\\Chair01.mdx;"), "the chair (display 39) has its model");
        assert!(all.split(';').filter(|s| !s.is_empty()).all(|s| matches!(s.split(',').count(), 5 | 6)));
        let m = |t: &str| AddonMsg { prefix: "OVMOD".into(), text: t.into() };
        let mut d = Den::new(den::Spot::default());
        let mut sel = Sel::default();
        let out = handle(&mut d, &mut sel, None, &m("wubuild\tcatalog\t1/1\t"), true, true);
        assert!(matches!(&out[0], Out::Addon(a, b, _) if a == "wubuild" && b == "catalog"));
        assert!(matches!(&out[1], Out::Addon(_, b, _) if b == "catmore"));
        assert_eq!(out.last(), Some(&Out::Ready));
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
    fn redo_copy_paste_duplicate_select_floor_grid_and_find() {
        let mut d = Den::new(den::Spot::default());
        let s0 = den::Spot::default();
        let here = Here { x: s0.x, y: s0.y, z: s0.z, o: 0.0 };
        let mut sel = Sel::default();
        builder_cmd(&mut d, &mut sel, here, "spawn 2413", true);
        let g = sel.guid.unwrap();
        builder_cmd(&mut d, &mut sel, here, "turn 90", true);
        builder_cmd(&mut d, &mut sel, here, "undo", true);
        assert!(state(&d, &sel, "owner").contains("redo=1;"), "Redo 1 after an undo");
        builder_cmd(&mut d, &mut sel, here, "redo", true);
        assert_eq!(d.placed(g).unwrap().prop.yaw, 90.0, "the turn redone");
        builder_cmd(&mut d, &mut sel, here, "undo", true);
        builder_cmd(&mut d, &mut sel, here, "undo", true);
        assert!(d.props.is_empty());
        let o = builder_cmd(&mut d, &mut sel, here, "redo", true);
        assert!(matches!(&o[0], Out::Change(c) if c.created.len() == 1), "the place redone: {o:?}");
        assert_eq!(d.props.len(), 1);
        let o = builder_cmd(&mut d, &mut sel, here, "redo", true);
        assert!(matches!(&o[0], Out::Change(c) if c.created.is_empty()), "the turn is gone with the old object");
        // a new change empties Redo
        builder_cmd(&mut d, &mut sel, here, "selall", true);
        builder_cmd(&mut d, &mut sel, here, "move z 1", true);
        builder_cmd(&mut d, &mut sel, here, "undo", true);
        builder_cmd(&mut d, &mut sel, here, "move z 2", true);
        assert_eq!(d.redo_depth(), 0);
        // copy, paste, duplicate
        builder_cmd(&mut d, &mut sel, here, "copy", true);
        assert!(state(&d, &sel, "owner").contains("clip=1;"));
        let o = builder_cmd(&mut d, &mut sel, here, "paste", true);
        assert!(matches!(&o[0], Out::Change(c) if c.created.len() == 1), "{o:?}");
        assert_eq!(d.props.len(), 2);
        assert!((d.props[1].prop.z - 2.0).abs() < 1e-6, "as high as the copied one");
        builder_cmd(&mut d, &mut sel, here, "dup", true);
        assert_eq!(d.props.len(), 3);
        assert!((d.props[2].prop.x - d.props[1].prop.x - 1.0).abs() < 1e-6, "a yard along");
        builder_cmd(&mut d, &mut sel, here, "floor", true);
        assert_eq!(d.props[2].prop.z, 0.0, "down on the floor");
        // pick by id, the grid, find
        let g = d.props[0].obj.guid;
        let id = prop_id(g);
        let o = builder_cmd(&mut d, &mut sel, here, &format!("pick {id}"), true);
        assert!(matches!(&o[0], Out::Say(s) if s.starts_with("Selected")) && sel.guid == Some(g));
        builder_cmd(&mut d, &mut sel, here, "grid 2", true);
        assert_eq!(sel.grid, 2.0);
        builder_cmd(&mut d, &mut sel, Here { x: s0.x + 0.7, ..here }, "spawn 2413", true);
        let p = &d.placed(sel.guid.unwrap()).unwrap().prop;
        assert!(((p.x / 2.0).round() * 2.0 - p.x).abs() < 0.01 && ((p.y / 2.0).round() * 2.0 - p.y).abs() < 0.01, "on the 2 yard grid: {p:?}");
        let o = builder_cmd(&mut d, &mut sel, here, "find chair", true);
        assert!(matches!(&o[0], Out::Addon(a, b, c) if a == "wubuild" && b == "found" && c.starts_with("objects,1,0;") && c.contains(",Wooden Chair,")), "{o:?}");
        // group is Den Lite's own since 4 Nov (buildtools.rs, which world.rs asks first); stack is still the full Den's
        let o = builder_cmd(&mut d, &mut sel, here, "stack", true);
        assert!(matches!(&o[0], Out::Say(s) if s.contains("full Den")));
        assert!(state(&d, &sel, "owner").contains(",tool,path,array,"), "the tools' caps");
        assert!(state(&d, &sel, "owner").contains("lite=1;grid=2;"));
        let m = AddonMsg { prefix: "OVMOD".into(), text: "wuden\treport\t1/1\tgallery tiles=20 model=18".into() };
        assert_eq!(handle(&mut d, &mut sel, None, &m, true, true), vec![Out::Report("gallery tiles=20 model=18".into())]);
    }

    #[test]
    fn select_next_every_one_of_a_kind_lock_and_the_caps() {
        let mut d = Den::new(den::Spot::default());
        let s0 = den::Spot::default();
        let here = Here { x: s0.x, y: s0.y, z: s0.z, o: 0.0 };
        let mut sel = Sel::default();
        // three chairs and a barrel, 1, 2, 4 and 3 yards along x
        let a = d.spawn(2413, s0.x + 1.0, s0.y, s0.z, 0.0).unwrap().guid;
        let b = d.spawn(2413, s0.x + 2.0, s0.y, s0.z, 0.0).unwrap().guid;
        let c = d.spawn(2413, s0.x + 4.0, s0.y, s0.z, 0.0).unwrap().guid;
        let barrel = crate::objects::OBJECTS.iter().find(|o| o.3.to_lowercase().contains("barrel")).unwrap().0;
        let k = d.spawn(barrel, s0.x + 3.0, s0.y, s0.z, 0.0).unwrap().guid;
        // the round from the selection, by distance, wrapping
        sel.guid = Some(a);
        let mut seen = Vec::new();
        for _ in 0..5 {
            builder_cmd(&mut d, &mut sel, here, "selnext", true);
            seen.push(sel.guid.unwrap());
        }
        assert_eq!(seen, vec![b, k, c, a, b], "nearest first from where the round started, then around again");
        // every chair: Delete and moves take them all
        sel.guid = Some(a);
        let o = builder_cmd(&mut d, &mut sel, here, "selkind", true);
        assert!(matches!(&o[0], Out::Say(t) if t.contains("3 props")), "{o:?}");
        let st = state(&d, &sel, "owner");
        assert!(st.contains("n=3;") && st.contains(&format!("ids={},{},{};", prop_id(a), prop_id(b), prop_id(c))), "{st}");
        builder_cmd(&mut d, &mut sel, here, "move z 1", true);
        assert!([a, b, c].iter().all(|g| (d.placed(*g).unwrap().prop.z - 1.0).abs() < 1e-6), "all three up");
        assert_eq!(d.placed(k).unwrap().prop.z, 0.0, "not the barrel");
        // lock: no move, no delete, kept in the file; unlock
        sel.more.clear();
        sel.guid = Some(a);
        builder_cmd(&mut d, &mut sel, here, "lock", true);
        assert!(d.locked(a) && state(&d, &sel, "owner").contains("locked=1;"));
        let o = builder_cmd(&mut d, &mut sel, here, "move x 1", true);
        assert!(matches!(&o[0], Out::Say(t) if t == den::LOCKED), "{o:?}");
        let o = builder_cmd(&mut d, &mut sel, here, "delete", true);
        assert!(matches!(&o[0], Out::Say(t) if t == den::LOCKED) && d.placed(a).is_some());
        assert!(d.to_doc("x", 0).props.iter().any(|p| p.locked == Some(true)), "in the Den file");
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "unlock", false)[0], Out::Say(t) if t.contains("owner")), "a visitor can't");
        builder_cmd(&mut d, &mut sel, here, "unlock", true);
        assert!(!d.locked(a));
        builder_cmd(&mut d, &mut sel, here, "delete", true);
        assert!(d.placed(a).is_none());
        // any other selection empties the kind and the round
        sel.guid = Some(b);
        builder_cmd(&mut d, &mut sel, here, "selkind", true);
        builder_cmd(&mut d, &mut sel, here, &format!("pick {}", prop_id(k)), true);
        assert!(sel.more.is_empty() && sel.round.is_none());
        // the caps: every verb this Den Lite answers
        let st = state(&d, &sel, "owner");
        let caps: Vec<&str> = st.split("caps=").nth(1).unwrap().split(';').next().unwrap().split(',').collect();
        for v in ["selnext", "selkind", "lock", "unlock", "preview", "hover", "commit", "axis", "gizmo", "mode", "spawn", "delete"] {
            assert!(caps.contains(&v), "{v} in caps");
        }
        assert!(!FULL_ONLY.iter().any(|v| caps.contains(v)), "nothing only the full Den does");
    }

    #[test]
    fn the_selection_panels_pad_quick_buttons_and_swap() {
        let mut d = Den::new(den::Spot::default());
        let s0 = den::Spot::default();
        let here = Here { x: s0.x, y: s0.y, z: s0.z, o: 0.0 };
        let mut sel = Sel::default();
        let a = d.spawn(2413, s0.x + 3.0, s0.y, s0.z, 0.0).unwrap().guid;
        let at = |d: &Den| d.placed(a).map(|p| (p.obj.x - s0.x, p.obj.y - s0.y, p.obj.z - d.floor_at(p.obj.x, p.obj.y))).unwrap();
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "step fwd 1", true)[0], Out::Say(t) if t.contains("Select")));
        sel.guid = Some(a);
        // the pad, from where you stand (facing +x): forward, left, up; one undo step each
        builder_cmd(&mut d, &mut sel, here, "step fwd 1", true);
        builder_cmd(&mut d, &mut sel, here, "step left 2", true);
        builder_cmd(&mut d, &mut sel, here, "step up 0.5", true);
        let (x, y, z) = at(&d);
        assert!((x - 4.0).abs() < 0.01 && (y - 2.0).abs() < 0.01 && (z - 0.5).abs() < 0.01, "{x} {y} {z}");
        assert_eq!(step_offset("right", std::f32::consts::FRAC_PI_2, 1.0).map(|v| (v.0.round(), v.1.round())), Some((1.0, -0.0)));
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "step sideways 1", true)[0], Out::Say(t) if t.contains("Usage")));
        // End: down to the ground; bigger, then Reset (straight, no turn, size 1)
        builder_cmd(&mut d, &mut sel, here, "ground", true);
        assert!(at(&d).2.abs() < 0.01);
        builder_cmd(&mut d, &mut sel, here, "grow 1.25", true);
        assert_eq!(d.placed(a).unwrap().prop.scale, Some(1.25));
        builder_cmd(&mut d, &mut sel, here, "turn 30", true);
        builder_cmd(&mut d, &mut sel, here, "reset", true);
        let p = d.placed(a).unwrap().prop.clone();
        assert!(p.scale.is_none() && p.yaw.abs() < 0.01);
        builder_cmd(&mut d, &mut sel, here, "random", true);
        assert!((0.0..360.0).contains(&d.placed(a).unwrap().prop.yaw));
        // Home: in front of you; the centre: the Den's middle
        builder_cmd(&mut d, &mut sel, Here { x: s0.x + 10.0, ..here }, "bring", true);
        assert!((at(&d).0 - 13.0).abs() < 0.01);
        builder_cmd(&mut d, &mut sel, here, "centre", true);
        assert!(at(&d).0.abs() < 0.01 && at(&d).1.abs() < 0.01);
        // Swap with: the selection becomes a barrel, where it was; undo takes it back
        let barrel = crate::objects::OBJECTS.iter().find(|o| o.3.to_lowercase().contains("barrel")).unwrap().0;
        assert!(matches!(&builder_cmd(&mut d, &mut sel, here, "swap 7", true)[0], Out::Say(t) if t.contains("gallery")));
        let o = builder_cmd(&mut d, &mut sel, here, &format!("swap {barrel}"), true);
        assert!(o.iter().any(|x| matches!(x, Out::Change(c) if c.created.len() == 1 && c.message.starts_with("Swapped for"))));
        assert_eq!(d.placed(a).unwrap().prop.entry, barrel as u64);
        d.undo();
        assert_eq!(d.placed(a).unwrap().prop.entry, 2413);
        // All of this kind: every chair, one undo for them all; Shift+right-click adds to the selection
        let b = d.spawn(2413, s0.x - 2.0, s0.y, s0.z, 0.0).unwrap().guid;
        let c = d.spawn(2413, s0.x - 4.0, s0.y, s0.z, 0.0).unwrap().guid;
        let o = builder_cmd(&mut d, &mut sel, here, &format!("swapkind {barrel}"), true);
        assert!(o.iter().any(|x| matches!(x, Out::Change(ch) if ch.created.len() == 3 && ch.message.contains("3 props"))));
        assert!([a, b, c].iter().all(|g| d.placed(*g).unwrap().prop.entry == barrel as u64));
        let msg = d.undo().message;
        assert!([a, b, c].iter().all(|g| d.placed(*g).unwrap().prop.entry == 2413), "one undo: {msg}");
        assert!(d.redo().message.contains("Redone") && d.placed(c).unwrap().prop.entry == barrel as u64);
        builder_cmd(&mut d, &mut sel, here, &format!("seladd {}", prop_id(b)), true);
        assert_eq!((sel.guid, sel.more.clone()), (Some(a), vec![b]));
        let caps: Vec<&str> = CAPS.split(',').collect();
        for v in ["swap", "swapkind", "bring", "ground", "settle", "random", "centre", "reset", "step", "grow", "seladd", "grab", "cam", "click", "drag"] {
            assert!(caps.contains(&v), "{v} in caps");
        }
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
        assert_eq!(handle(&mut d, &mut sel, None, &m("wuden\thello\t1/1\t1"), true, true), vec![Out::Hello, Out::Ready], "the Den bar");
        assert_eq!(handle(&mut d, &mut sel, None, &m("wuden\tstatus\t1/1\t"), false, false), vec![Out::Status]);
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
