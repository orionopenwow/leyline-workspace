//! Tales in the Den (tale.rs is the script and its player; this is the Den's side): where tales come from, the NPCs,
//! items, signs and portals a tale puts in the world, what its lines and steps look like in game, Jaina writing one
//! with the player (their own assistant when they connected one on the launcher's Agents page, else her own built-in
//! drafts), Sky Island's edge, and the rewards kept (looks only).
//!
//! Everything here is worked out under the `tales` lock and handed to the world server as [`Out`]s (world.rs
//! `tale_outs` sends them), so this module never writes to a game itself and never holds two locks at once.
//!
//! Where tales come from (`.den tale list`):
//! * the Den's own (`tales` in its file: they travel with the Den file and its code);
//! * `<dens>/tales/<name>.tale` (Jaina's drafts the player kept, a file the player wrote);
//! * a Den file whose name starts `tale-` (what the launcher makes of a tale's code: ui/denpack.js).

use crate::companion::At;
use crate::den::WorldObject;
use crate::skyisland;
use crate::tale::{self, Act, Ev, Kind, Reward, Run};
use crate::{Lock, Shared};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use wow_world_messages::wrath::*;
use wow_world_messages::Guid;

/// What the world server does for a tale, in this order.
pub enum Out {
    /// A system line to the host.
    Say(String),
    /// A line in someone's voice (an NPC's, or Jaina's: her guid and name).
    Voice { name: String, guid: u64, text: String },
    /// `wuden <name> <data>` to the host's Leyline UI (the tale tracker: DenTale.lua).
    Addon(&'static str, String),
    /// An NPC (a unit) to make for the host.
    Unit(Box<SMSG_UPDATE_OBJECT>),
    /// Gameobjects to show the host (a tale's items, signs, portals).
    Objects(Vec<WorldObject>),
    Destroy(Vec<u64>),
    /// A near teleport of the host: x, y, z, facing (world).
    Teleport([f32; 4]),
    /// The events log.
    Event(&'static str, String),
    /// Training dummies by this spot (Sky Island's), this many.
    Dummies(At, usize),
}

/// A draft Jaina shows before anything plays.
#[derive(Debug, Clone)]
pub struct Draft {
    pub text: String,
    pub tale: tale::Tale,
    /// "jaina" (her own) or "assistant" (the player's own, through the Agents page).
    pub by: &'static str,
}

#[derive(Debug, Default)]
pub struct Tales {
    pub run: Option<Run>,
    /// The tale's text as it started (for `.den tale code` and `keep`).
    pub text: String,
    /// What is shown for it: thing id -> guid.
    pub shown: Vec<(String, u64)>,
    pub draft: Option<Draft>,
    /// The player's words sent to their assistant for a draft, and when (ms): Jaina's own draft after [`ASK_WAIT_MS`].
    pub asked: Option<(String, u64)>,
    /// The last assistant draft read (tale-draft.json's `n`).
    pub seen_draft: u64,
    /// What happened since the last tick (a dummy down).
    pub inbox: Vec<Ev>,
    last_tick: u64,
    last_rescue: u64,
}

/// The assistant has this long to send a draft (ms), then Jaina offers her own.
pub const ASK_WAIT_MS: u64 = 60_000;
/// The file the launcher's `den_tale_draft` tool writes (agents/den.rs): `{"n": <ms>, "text": "<tale>"}`.
pub const DRAFT_FILE: &str = "tale-draft.json";
pub const TALES_DIR: &str = "tales";
pub const REWARDS_FILE: &str = "tale-rewards.json";
/// A tale's NPCs: creature entries 990 060 + their place in the tale (their names: this server's creature query).
pub const NPC_ENTRY: u32 = 990_060;
/// A tale's gameobjects: their guids' counters from here (never a Den prop's or an aid's).
const THING_COUNTER: u64 = 0xFE_0000;
const HIGH_GAMEOBJECT: u64 = 0xF110;
const HIGH_CREATURE: u64 = 0xF130;
/// Rescues at most this often (ms).
const RESCUE_MS: u64 = 1_500;

/// The NPC looks: the game's own creatures (AzerothCore entries, their models from creatures.rs), a fallback model.
pub const LOOK_CREATURES: &[(&str, u32, u32)] = &[
    ("villager", 295, 27510),
    ("guard", 68, 27510),
    ("mage", 18166, crate::companion::DISPLAY),
    ("dwarf", 2784, 27510),
    ("elf", 7999, 27510),
    ("ghost", 6491, 27510),
    ("dummy", 31144, 27510),
    ("jaina", crate::companion::ENTRY, crate::companion::DISPLAY),
];

pub fn look_display(look: &str) -> u32 {
    let (_, entry, fallback) = LOOK_CREATURES.iter().copied().find(|l| l.0 == look).unwrap_or(LOOK_CREATURES[0]);
    crate::creatures::display(entry).unwrap_or(fallback)
}

/// The names of the NPCs shown now, by entry (the creature query answers them; one Den Lite has one tale at a time,
/// and a test's other servers only ever add the same names).
static NAMES: Mutex<Vec<(u32, String)>> = Mutex::new(Vec::new());

pub fn npc_guid(slot: usize) -> u64 {
    (HIGH_CREATURE << 48) | (((NPC_ENTRY + slot as u32) as u64) << 24) | 1
}

fn thing_guid(entry: u32, slot: usize) -> u64 {
    (HIGH_GAMEOBJECT << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (THING_COUNTER + slot as u64)
}

/// CMSG_CREATURE_QUERY for a tale's NPC.
pub fn creature(entry: u32) -> Option<SMSG_CREATURE_QUERY_RESPONSE> {
    if !(NPC_ENTRY..NPC_ENTRY + tale::MAX_THINGS as u32).contains(&entry) {
        return None;
    }
    let name = NAMES.lk().iter().find(|n| n.0 == entry).map(|n| n.1.clone()).unwrap_or_default();
    Some(SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: name,
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: "<A tale>".into(),
            description: String::new(),
            type_flags: 0,
            creature_type: 7, // humanoid
            creature_family: CreatureFamily::None,
            creature_rank: 0,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [27510, 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

/// An NPC's create: friendly, can be targeted (its name shows), can't be attacked.
fn npc_create(slot: usize, display: u32, at: At) -> SMSG_UPDATE_OBJECT {
    let guid = Guid::new(npc_guid(slot));
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(guid)
        .set_object_entry((NPC_ENTRY + slot as u32) as i32)
        .set_object_scale_x(1.0)
        .set_unit_bytes_0(Race::Human, Class::Warrior, Gender::Male, Power::Rage)
        .set_unit_health(100)
        .set_unit_maxhealth(100)
        .set_unit_level(60)
        .set_unit_factiontemplate(35)
        .set_unit_flags((0x0000_0002 | 0x0000_0100 | 0x0000_0200) as i32)
        .set_unit_boundingradius(0.4)
        .set_unit_combatreach(1.5)
        .set_unit_displayid(display as i32)
        .set_unit_nativedisplayid(display as i32)
        .set_unit_bytes_1(UnitStandState::Stand, 0, 0, 0)
        .set_unit_mod_cast_speed(1.0)
        .set_unit_npc_flags(0)
        .set_unit_hoverheight(1.0)
        .finalize();
    let living = MovementBlock_UpdateFlag_Living::Living {
        backwards_flight_speed: 4.5,
        backwards_running_speed: 4.5,
        backwards_swimming_speed: 4.5,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 7.0,
        orientation: at.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: at.x, y: at.y, z: at.z },
        running_speed: 7.0,
        swimming_speed: 4.7,
        timestamp: 0,
        turn_rate: std::f32::consts::PI,
        walking_speed: 2.5,
    };
    SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject {
            guid3: guid,
            mask2: UpdateMask::Unit(mask),
            movement2: MovementBlock { update_flag: MovementBlock_UpdateFlag::empty().set_living(living) },
            object_type: ObjectType::Unit,
        }],
    }
}

/// The gameobject a tale's item, sign or portal is shown as (a catalog prop: the player's own game's model).
fn thing_object(x: &tale::Thing, slot: usize, spot: crate::den::Spot, floor: f32) -> Option<WorldObject> {
    let word = match &x.kind {
        Kind::Item { prop, .. } => prop.as_str(),
        Kind::Sign { .. } => "sign post",
        Kind::Portal { .. } => "meeting stone",
        Kind::Npc { .. } => return None,
    };
    let hit = crate::den::find_objects(word).into_iter().next().or_else(|| crate::den::find_objects("crate").into_iter().next())?;
    let t = crate::den::template(hit.0)?;
    Some(WorldObject {
        guid: thing_guid(t.entry, slot),
        entry: t.entry,
        display: t.display,
        kind: crate::gizmo::GENERIC,
        scale: t.size,
        x: spot.x + x.at[0],
        y: spot.y + x.at[1],
        z: floor,
        o: x.face.to_radians(),
        pitch: 0.0,
        roll: 0.0,
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The names of the tale's speakers (for a voice line).
fn speaker(t: &tale::Tale, who: &Option<String>) -> Option<(String, u64)> {
    match who.as_deref() {
        None => None,
        Some(tale::JAINA) => Some((crate::companion::NAME.to_string(), crate::companion::GUID)),
        Some(id) => t.things.iter().enumerate().find(|(_, x)| x.id == id).and_then(|(i, x)| match &x.kind {
            Kind::Npc { name, .. } => Some((name.clone(), npc_guid(i))),
            _ => None,
        }),
    }
}

/// What the tale's acts look like in game.
fn outs_of(shared: &Shared, ts: &mut Tales, acts: Vec<Act>) -> Vec<Out> {
    let Some(run) = ts.run.as_ref() else { return Vec::new() };
    let tale = run.tale.clone();
    let (spot, floor_of) = {
        let d = shared.den.lk();
        (d.spot, d.spot.z)
    };
    let mut out = Vec::new();
    for a in acts {
        match a {
            Act::Say(l) => match speaker(&tale, &l.who) {
                Some((name, guid)) => out.push(Out::Voice { name, guid, text: l.text }),
                None => out.push(Out::Say(l.text)),
            },
            Act::Track { n, of, title, goal } => {
                out.push(Out::Addon("tale", format!("step|{n}|{of}|{}|{}", clip(&title), clip(&goal))));
                out.push(Out::Say(format!("Tale, step {n} of {of}: {title} ({goal}).")));
            }
            Act::Hint(h) => out.push(Out::Addon("tale", format!("hint|{}", clip(&h)))),
            Act::Show(id) => {
                if let Some((i, x)) = tale.things.iter().enumerate().find(|(_, x)| x.id == id) {
                    if !ts.shown.iter().any(|s| s.0 == id) {
                        if let Some(o) = thing_object(x, i, spot, floor_of) {
                            ts.shown.push((id.clone(), o.guid));
                            out.push(Out::Objects(vec![o]));
                        }
                    }
                }
            }
            Act::Hide(id) => {
                let gone: Vec<u64> = ts.shown.iter().filter(|s| s.0 == id).map(|s| s.1).collect();
                ts.shown.retain(|s| s.0 != id);
                if !gone.is_empty() {
                    out.push(Out::Destroy(gone));
                }
            }
            Act::Teleport(p) => out.push(Out::Teleport([spot.x + p[0], spot.y + p[1], floor_of + 0.2, 0.0])),
            Act::Reward(r) => {
                let line = keep_reward(&shared.cfg.den_dir, &r);
                out.push(Out::Addon("tale", format!("reward|{}", clip(&line))));
                out.push(Out::Say(line));
            }
            Act::Finished(title) => {
                out.push(Out::Addon("tale", "done".into()));
                out.push(Out::Event("tale_done", clip(&title)));
                out.push(Out::Say(format!("The tale is done: {title}. .den tale code makes it a code to pass on.")));
            }
        }
    }
    out
}

fn clip(s: &str) -> String {
    s.replace('|', "/").chars().take(120).collect()
}

/// A reward kept in `<dens>/tale-rewards.json` (titles and badges: looks only), and the line that says it.
fn keep_reward(dir: &Path, r: &Reward) -> String {
    let p = dir.join(REWARDS_FILE);
    let mut v: serde_json::Value = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({"titles": [], "badges": []}));
    let (key, word, line) = match r {
        Reward::Title(t) => ("titles", t.clone(), format!("You earned the title {t}.")),
        Reward::Badge(b) => ("badges", b.clone(), format!("You earned a badge: {b}.")),
        Reward::Cheer => return "Jaina cheers for you!".into(),
        Reward::Glow => return "You glow with the tale's light for a moment.".into(),
    };
    if let Some(list) = v.get_mut(key).and_then(|l| l.as_array_mut()) {
        if !list.iter().any(|x| x.as_str() == Some(word.as_str())) && list.len() < 200 {
            list.push(serde_json::json!(word));
        }
    }
    let _ = crate::settings::write_atomic(&p, serde_json::to_string_pretty(&v).unwrap_or_default().as_bytes());
    line
}

/// Everything a tale shows, taken away.
fn clear(ts: &mut Tales) -> Vec<Out> {
    let mut gone: Vec<u64> = ts.shown.drain(..).map(|s| s.1).collect();
    if let Some(r) = ts.run.take() {
        for (i, x) in r.tale.things.iter().enumerate() {
            if matches!(x.kind, Kind::Npc { .. }) {
                gone.push(npc_guid(i));
            }
        }
    }
    ts.text.clear();
    if gone.is_empty() { Vec::new() } else { vec![Out::Destroy(gone), Out::Addon("tale", "off".into())] }
}

/// A tale starts (the one under way stops first): its NPCs, items, signs and portals shown, step 1.
pub fn start(shared: &Shared, text: &str) -> Result<Vec<Out>, String> {
    let t = tale::parse(text).map_err(|p| format!("That tale has a problem: {p}"))?;
    let (spot, floor) = {
        let d = shared.den.lk();
        (d.spot, d.spot.z)
    };
    if t.things.iter().any(|x| (x.at[0].powi(2) + x.at[1].powi(2)).sqrt() > spot.reach()) {
        return Err(format!("Part of that tale is outside your Den ({:.0} yards from its middle at most): .den grow first.", spot.reach()));
    }
    let mut ts = shared.tales.lk();
    let mut out = clear(&mut ts);
    {
        let mut names = NAMES.lk();
        for (i, x) in t.things.iter().enumerate() {
            if let Kind::Npc { name, look } = &x.kind {
                let entry = NPC_ENTRY + i as u32;
                names.retain(|n| n.0 != entry);
                names.push((entry, name.clone()));
                let at = At { x: spot.x + x.at[0], y: spot.y + x.at[1], z: floor, o: x.face.to_radians() };
                out.push(Out::Unit(Box::new(npc_create(i, look_display(look), at))));
            }
        }
    }
    let mut objs = Vec::new();
    for (i, x) in t.things.iter().enumerate() {
        if let Some(o) = thing_object(x, i, spot, floor) {
            ts.shown.push((x.id.clone(), o.guid));
            objs.push(o);
        }
    }
    if !objs.is_empty() {
        out.push(Out::Objects(objs));
    }
    let title = t.title.clone();
    let mut run = Run::new(t, now_ms());
    let acts = run.begin();
    ts.run = Some(run);
    ts.text = text.to_string();
    out.push(Out::Event("tale_start", clip(&title)));
    out.extend(outs_of(shared, &mut ts, acts));
    Ok(out)
}

/// A player moved (world.rs, each move): Sky Island's edge (everyone: a friend falls off too), then the tale (the
/// host's).
pub fn moved(shared: &Shared, me: At, flying: bool, host: bool) -> Vec<Out> {
    let mut out = Vec::new();
    let (spot, island) = {
        let d = shared.den.lk();
        (d.spot, d.look.name == skyisland::LOOK)
    };
    let now = now_ms();
    if island {
        let rel = [me.x - spot.x, me.y - spot.y, me.z - spot.z];
        if let Some(r) = skyisland::rescue(rel, flying) {
            let mut ts = shared.tales.lk();
            if now.saturating_sub(ts.last_rescue) >= RESCUE_MS {
                ts.last_rescue = now;
                drop(ts);
                match r {
                    skyisland::Rescue::Landing => {
                        let l = skyisland::LANDING;
                        out.push(Out::Teleport([spot.x + l[0], spot.y + l[1], spot.z + l[2], l[3]]));
                        out.push(Out::Event("island_fall", format!("{:.0} {:.0} {:.0}", rel[0], rel[1], rel[2])));
                        out.push(Out::Say("You fell off the island: back by the tree.".into()));
                    }
                    skyisland::Rescue::OnTop(z) => {
                        out.push(Out::Teleport([me.x, me.y, spot.z + z, me.o]));
                        out.push(Out::Event("island_lift", format!("{:.0} {:.0} {:.1}", rel[0], rel[1], rel[2])));
                    }
                }
                return out;
            }
        }
    }
    if !host {
        return out;
    }
    let jaina = {
        let c = shared.companion.lk();
        c.out.then(|| c.pos(now))
    };
    let mut ts = shared.tales.lk();
    let Some(run) = ts.run.as_mut() else { return out };
    let ev = Ev::At { me: [me.x - spot.x, me.y - spot.y], jaina: jaina.map(|j| [j.x - spot.x, j.y - spot.y]) };
    let acts = run.on(&ev);
    out.extend(outs_of(shared, &mut ts, acts));
    out
}

/// A training dummy went down (lib.rs Shared::event, the combat's `dummy_down`).
pub fn defeated(shared: &Shared) {
    if let Ok(mut ts) = shared.tales.try_lock() {
        if ts.run.is_some() && ts.inbox.len() < 64 {
            ts.inbox.push(Ev::Defeated);
        }
    }
}

/// Every 50 ms (world.rs dojo_tick): the inbox, the tale's clock (at most 4 a second), the assistant's draft.
pub fn tick(shared: &Shared) -> Vec<Out> {
    let now = now_ms();
    let mut out = Vec::new();
    let mut ts = shared.tales.lk();
    if now.saturating_sub(ts.last_tick) < 250 && ts.inbox.is_empty() {
        return out;
    }
    ts.last_tick = now;
    let inbox: Vec<Ev> = ts.inbox.drain(..).collect();
    if let Some(run) = ts.run.as_mut() {
        let mut acts = Vec::new();
        for e in inbox {
            acts.extend(run.on(&e));
        }
        acts.extend(run.on(&Ev::Tick(now)));
        out.extend(outs_of(shared, &mut ts, acts));
        if ts.run.as_ref().is_some_and(|r| r.finished()) {
            // the tale's things stay a moment: they go with the next tale or `.den tale stop`
        }
    }
    // the player's assistant's draft (agents/den.rs den_tale_draft), or Jaina's own after the wait
    if let Some((words, at)) = ts.asked.clone() {
        let dir = &shared.cfg.den_dir;
        let file = std::fs::read_to_string(dir.join(DRAFT_FILE)).ok().filter(|t| t.len() <= 16 * 1024).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let fresh = file.as_ref().and_then(|v| Some((v.get("n")?.as_u64()?, v.get("text")?.as_str()?.to_string()))).filter(|(n, _)| *n > ts.seen_draft && *n + 5_000 >= at);
        if let Some((n, text)) = fresh {
            ts.seen_draft = n;
            ts.asked = None;
            match tale::parse(&text) {
                Ok(t) => out.extend(offer(&mut ts, Draft { text: tale::write(&t), tale: t, by: "assistant" })),
                Err(p) => {
                    out.push(Out::Voice { name: crate::companion::NAME.into(), guid: crate::companion::GUID, text: format!("Your assistant's tale had a problem ({p}). Here's one of mine instead.") });
                    out.extend(own_draft(shared, &mut ts, &words));
                }
            }
        } else if now.saturating_sub(at) >= ASK_WAIT_MS {
            ts.asked = None;
            out.push(Out::Voice { name: crate::companion::NAME.into(), guid: crate::companion::GUID, text: "No word from your assistant yet, so I wrote one myself.".into() });
            out.extend(own_draft(shared, &mut ts, &words));
        }
    }
    out
}

fn own_draft(shared: &Shared, ts: &mut Tales, words: &str) -> Vec<Out> {
    let room = {
        let d = shared.den.lk();
        if d.look.name == skyisland::LOOK { skyisland::RADIUS } else { d.spot.reach() - 2.0 }
    };
    let text = tale::draft(words, room);
    match tale::parse(&text) {
        Ok(t) => offer(ts, Draft { text, tale: t, by: "jaina" }),
        Err(_) => Vec::new(),
    }
}

/// Jaina shows a draft: its title and steps (three lines at most: her no-spam rule), and waits for yes or no.
fn offer(ts: &mut Tales, d: Draft) -> Vec<Out> {
    let j = |text: String| Out::Voice { name: crate::companion::NAME.into(), guid: crate::companion::GUID, text };
    let steps = tale::outline(&d.tale);
    let shown: Vec<String> = steps.iter().take(4).cloned().collect();
    let more = if steps.len() > 4 { format!(" (and {} more)", steps.len() - 4) } else { String::new() };
    let out = vec![
        j(format!("Here's a tale{}: \"{}\", {} steps.", if d.by == "assistant" { " from your assistant" } else { "" }, d.tale.title, steps.len())),
        j(format!("{}{more}", shown.join("; "))),
        j("Say \"Jaina, yes\" to play it, or \"Jaina, no\".".into()),
        Out::Addon("tale", format!("draft|{}|{}", clip(&d.tale.title), clip(&steps.join(" / ")))),
        Out::Event("tale_draft", format!("{} {}", d.by, clip(&d.tale.title))),
    ];
    ts.draft = Some(d);
    out
}

/// "Jaina, make a quest where ..." (or a tale, a story, an adventure).
pub fn wants_tale(words: &str) -> bool {
    let w = words.to_lowercase();
    let ask = ["make", "write", "create", "draft", "give me", "invent", "build me", "design"].iter().any(|k| w.contains(k));
    let what = ["quest", "tale", "story", "adventure", "questline"].iter().any(|k| w.contains(k));
    ask && what
}

/// "Jaina, ..." (world.rs, before her other words): a tale asked for, or the answer to a draft. None: not hers.
pub fn heard(shared: &Shared, words: &str) -> Option<Vec<Out>> {
    let low = words.trim().to_lowercase();
    let pending = shared.tales.lk().draft.is_some();
    if pending && matches!(crate::jaina::word(&low), Some(crate::jaina::Word::Yes)) {
        return Some(accept(shared));
    }
    if pending && matches!(crate::jaina::word(&low), Some(crate::jaina::Word::No)) {
        shared.tales.lk().draft = None;
        return Some(vec![Out::Voice { name: crate::companion::NAME.into(), guid: crate::companion::GUID, text: "All right, I'll put that one away.".into() }, Out::Addon("tale", "draft-off".into())]);
    }
    if !wants_tale(&low) {
        return None;
    }
    Some(ask(shared, words))
}

/// A tale asked for: to the player's own assistant when they connected one and it is listening (the Agents page,
/// never our keys; the words go nowhere else), else Jaina writes it herself at once.
pub fn ask(shared: &Shared, words: &str) -> Vec<Out> {
    let dir = shared.cfg.den_dir.clone();
    let now = now_ms();
    let unix = now / 1000;
    let listening = crate::companion::Companion::listening(&dir, unix);
    if listening {
        let asked = {
            let mut c = shared.companion.lk();
            c.ask(&dir, &format!("Please write a Den tale (use den_tale_draft): {words}"), None, unix)
        };
        if asked.is_ok() {
            shared.tales.lk().asked = Some((words.to_string(), now));
            return vec![
                Out::Voice { name: crate::companion::NAME.into(), guid: crate::companion::GUID, text: "Let me ask your assistant to write that tale. One moment...".into() },
                Out::Event("tale_ask", "assistant".into()),
            ];
        }
    }
    let mut ts = shared.tales.lk();
    own_draft(shared, &mut ts, words)
}

/// "Jaina, yes" to a draft: kept in `<dens>/tales/` and played.
pub fn accept(shared: &Shared) -> Vec<Out> {
    let Some(d) = shared.tales.lk().draft.take() else { return Vec::new() };
    let name = slug(&d.tale.title);
    let dir = shared.cfg.den_dir.join(TALES_DIR);
    let _ = std::fs::create_dir_all(&dir);
    let _ = crate::settings::write_atomic(&dir.join(format!("{name}.tale")), d.text.as_bytes());
    let mut out = vec![Out::Addon("tale", "draft-off".into())];
    match start(shared, &d.text) {
        Ok(o) => out.extend(o),
        Err(e) => out.push(Out::Say(e)),
    }
    out
}

/// A file name from a title: a-z 0-9 and -, 1 to 24.
pub fn slug(title: &str) -> String {
    let s: String = title.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let s: String = s.chars().take(24).collect();
    if s.is_empty() { "tale".into() } else { s.trim_end_matches('-').to_string() }
}

/// The tales there are: (name, text). The Den's own first (`den-1`...), then the kept files, then `tale-` Den files.
pub fn list(shared: &Shared) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let own: Vec<String> = shared.den.lk().extra.get("tales").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).take(16).collect()).unwrap_or_default();
    for (i, t) in own.into_iter().enumerate() {
        out.push((format!("den-{}", i + 1), t));
    }
    let dir = shared.cfg.den_dir.join(TALES_DIR);
    let mut files: Vec<_> = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    files.sort();
    for p in files.into_iter().filter(|p| p.extension().is_some_and(|e| e == "tale")).take(100) {
        if std::fs::metadata(&p).map(|m| m.len() as usize <= tale::MAX_BYTES).unwrap_or(false) {
            if let (Some(n), Ok(t)) = (p.file_stem().and_then(|s| s.to_str()), std::fs::read_to_string(&p)) {
                out.push((n.to_string(), t));
            }
        }
    }
    for n in crate::den::list_names(&shared.cfg.den_dir).into_iter().filter(|n| n.starts_with("tale-")).take(50) {
        if let Ok(doc) = crate::den::read_file(&shared.cfg.den_dir.join(format!("{n}.den.json"))) {
            if let Some(t) = doc.extra.get("tales").and_then(|v| v.as_array()).and_then(|a| a.first()).and_then(|t| t.as_str()) {
                out.push((n, t.to_string()));
            }
        }
    }
    out
}

/// `.den tale [list|play <name>|stop|code|keep|draft <words>|use <id>|yes|no|status|help]` (the host's).
pub fn chat(shared: &Shared, args: &[String], raw_rest: &str) -> Vec<Out> {
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let say = |s: &str| vec![Out::Say(s.to_string())];
    match a(0) {
        "" | "help" => vec![
            Out::Say(".den tale list: the tales here; .den tale play <name>: play one; .den tale stop; .den tale status".into()),
            Out::Say("Say \"Jaina, make a quest where ...\" and she writes one with you; .den tale keep puts the tale in your Den's file; .den tale code: a code to pass it on".into()),
        ],
        "list" => {
            let l = list(shared);
            if l.is_empty() {
                return say("No tales here yet. Say \"Jaina, make a quest where you find my lost lantern\".");
            }
            let names: Vec<String> = l.iter().take(20).map(|(n, t)| format!("{n} ({})", tale::parse(t).map(|t| t.title).unwrap_or_else(|_| "has a problem".into()))).collect();
            say(&format!("Tales: {}. .den tale play <name>", names.join(", ")))
        }
        "play" | "start" => {
            let want = a(1);
            let l = list(shared);
            let pick = if want.is_empty() { l.first().cloned() } else { l.iter().find(|(n, _)| n == want).cloned() };
            match pick {
                None => say("No such tale: .den tale list shows them."),
                Some((_, text)) => start(shared, &text).unwrap_or_else(|e| say(&e)),
            }
        }
        "stop" | "end" | "off" => {
            let mut ts = shared.tales.lk();
            let mut o = clear(&mut ts);
            o.push(Out::Say("The tale stopped.".into()));
            o
        }
        "status" => {
            let ts = shared.tales.lk();
            match ts.run.as_ref() {
                None => say("No tale under way."),
                Some(r) if r.finished() => say(&format!("{}: done.", r.tale.title)),
                Some(r) => {
                    let s = &r.tale.steps[r.step];
                    say(&format!("{}: step {} of {}, {} ({}).", r.tale.title, r.step + 1, r.tale.steps.len(), s.title, tale::goal_words(&r.tale, &s.goal)))
                }
            }
        }
        "use" | "talk" => {
            let id = a(1).to_string();
            let mut ts = shared.tales.lk();
            let Some(run) = ts.run.as_mut() else { return say("No tale under way.") };
            let acts = run.on(&Ev::Use(id));
            outs_of(shared, &mut ts, acts)
        }
        "keep" => {
            let text = {
                let ts = shared.tales.lk();
                if !ts.text.is_empty() { ts.text.clone() } else { ts.draft.as_ref().map(|d| d.text.clone()).unwrap_or_default() }
            };
            if text.is_empty() {
                return say("Play or draft a tale first.");
            }
            let mut d = shared.den.lk();
            let list = d.extra.entry("tales").or_insert_with(|| serde_json::json!([]));
            if let Some(arr) = list.as_array_mut() {
                if !arr.iter().any(|t| t.as_str() == Some(text.as_str())) {
                    if arr.len() >= 16 {
                        return say("Your Den keeps up to 16 tales.");
                    }
                    arr.push(serde_json::json!(text));
                }
            }
            drop(d);
            shared.autosave();
            say("The tale is in your Den now: it goes with its file and its code (.den pack).")
        }
        "code" | "share" => {
            let text = shared.tales.lk().text.clone();
            let text = if text.is_empty() { list(shared).first().map(|t| t.1.clone()).unwrap_or_default() } else { text };
            if text.is_empty() {
                return say("Play a tale first (.den tale list).");
            }
            let code = crate::packs::code_of(crate::denpack::Kind::Tale, &text);
            let title = tale::parse(&text).map(|t| t.title).unwrap_or_else(|_| "tale".into());
            match crate::packs::keep(&shared.cfg.den_dir, &format!("tale-{}", slug(&title)), &code) {
                Ok(p) => vec![
                    Out::Addon("pack", crate::denpack::link(&code).unwrap_or_else(|| code.clone())),
                    Out::Say(format!("The tale's code is in {} ({} letters): the launcher's Saved Dens > Open a code takes it, or a friend's.", p.display(), code.len())),
                ],
                Err(e) => say(&e),
            }
        }
        "draft" | "make" | "write" => {
            let words = raw_rest.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
            if words.is_empty() {
                return say("Draft what? .den tale draft find my lost lantern, then beat 3 dummies");
            }
            ask(shared, &words)
        }
        "yes" => accept(shared),
        "no" => {
            shared.tales.lk().draft = None;
            vec![Out::Addon("tale", "draft-off".into())]
        }
        _ => say("Usage: .den tale [list|play <name>|stop|status|keep|code|draft <words>|use <id>]"),
    }
}

/// The island: `.den island` takes your Den there (the Den here kept for this spot, as `.den here` does), lands you by
/// the tree, puts up the dummies and starts its welcome tale the first time. Returns the Den's change, where to land,
/// and what to say. `.den island off`: the Den back on its first spot.
pub fn island_doc() -> crate::den::DenDoc {
    crate::den::parse(&skyisland::den_json("sky-island")).expect("the island's own file")
}

/// Whether the welcome tale was finished here before (the rewards file holds its title).
pub fn welcomed(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join(REWARDS_FILE)).ok().is_some_and(|t| t.contains("Sky Walker"))
}

/// Names in the NPC table (a test reads them).
pub fn names() -> HashMap<u32, String> {
    NAMES.lk().iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asking_for_a_tale_and_slugs() {
        assert!(wants_tale("make a quest where you find my cat"));
        assert!(wants_tale("Write me a little story"));
        assert!(!wants_tale("build a garden"));
        assert!(!wants_tale("quest"));
        assert_eq!(slug("The Lost Lantern!"), "the-lost-lantern");
        assert_eq!(slug("!!!"), "tale");
        assert!(slug(&"a".repeat(40)).len() <= 24);
        assert_eq!(look_display("nobody"), look_display("villager"));
        assert!(creature(NPC_ENTRY).is_some() && creature(NPC_ENTRY + 99).is_none());
        assert_ne!(npc_guid(0), npc_guid(1));
        assert_ne!(thing_guid(19626, 0), thing_guid(19626, 1));
        assert_eq!(island_doc().room.unwrap().look, skyisland::LOOK);
    }
}
