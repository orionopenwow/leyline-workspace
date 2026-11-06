//! Boss Rush's light version in the world (a child module of world.rs, so its hooks there stay a few lines): the
//! fight of bossrush.rs at its real spot on its real map. `.den bossrush <fight> [normal|heroic] [class spec]` (or the
//! launcher's Boss Rush page, through `<dens>/bossrush.json`) makes the hero that class and spec at 80, sends the
//! player to the fight's platform (the game draws it from its own files), and once they are there puts the boss, nine
//! companions and the fight's adds up as creatures. The player's own spells and swings hit the boss (combat.rs
//! `Kind::Raid`); the fight's rings and cones are shown as casts at their spot; its hits take the player's own
//! health, down to 0 (a wipe). A clear or a wipe goes on the launcher's board (`<dens>/bossrush-results.tsv`), then the
//! fight is ready again in a few seconds (Try again: hit the boss).

use super::*;
use crate::bossrush::{self as br, Ev, Live, State};
use crate::combat;

/// Companions' entries (names Den Lite answers; never a real creature's).
const MEMBER_ENTRY: u32 = 905_000;
const BOSS_LEVEL: u32 = 83;
const READY_AGAIN_MS: u64 = 6000;

fn names(entry: u32) -> Option<&'static str> {
    Some(match entry {
        36597 => "The Frozen King",
        36612 => "The Bone Lord",
        36855 => "The Cult Mistress",
        39863 => "The Twilight Destroyer",
        28860 => "The Obsidian Dragon",
        10184 => "The Brood Mother",
        31125 => "The Stone Watcher",
        37695 => "Risen Ghoul",
        36619 => "Bone Spike",
        37949 => "Cultist",
        11262 => "Whelp",
        _ => return None,
    })
}

/// The boss, the companions and the adds as combat.rs's units (the player's spells and swings reach them).
pub(super) fn units(shared: &Shared) -> Vec<combat::Unit> {
    let r = shared.rush.lk();
    let list = match r.live.as_ref() {
        Some(l) if l.spawned => unit_list(l),
        Some(_) => Vec::new(),
        None => r.cache.clone(), // a beat is being worked on (tick has the fight out): the last beat's
    };
    drop(r);
    list.into_iter()
        .map(|(guid, s, hostile)| combat::Unit {
            guid,
            at: At { x: s.x, y: s.y, z: s.z, o: s.o },
            kind: if hostile { combat::Kind::Raid } else { combat::Kind::Friend },
            level: if guid == br::BOSS_GUID { BOSS_LEVEL } else { 80 },
        })
        .collect()
}

fn unit_list(l: &Live) -> Vec<(u64, br::Spot, bool)> {
    let f = &l.fight;
    let mut v = Vec::new();
    if f.state != State::Cleared {
        v.push((br::BOSS_GUID, f.boss_at, true));
    }
    v.extend(f.adds.iter().map(|a| (a.guid, a.at, true)));
    v.extend(f.members.iter().filter(|m| m.bot).map(|m| (m.guid(), m.at, false)));
    v
}

/// CMSG_CREATURE_QUERY for the fight's creatures: our own names (never the game's: no boss's own name shows).
pub(super) fn answer(entry: u32) -> Option<SMSG_CREATURE_QUERY_RESPONSE> {
    let (name, display, rank, sub) = if (MEMBER_ENTRY..MEMBER_ENTRY + 10).contains(&entry) {
        let i = (entry - MEMBER_ENTRY) as usize;
        (br::NAMES.get(i.saturating_sub(1)).copied().unwrap_or("Companion"), br::MEMBER_DISPLAYS[i % 2], 0, "Companion")
    } else {
        let n = names(entry)?;
        let boss = br::CATALOG.iter().any(|(k, _)| br::encounter(k).is_some_and(|d| d.boss == entry));
        (n, crate::creatures::display(entry).unwrap_or(0), if boss { 3 } else { 1 }, "")
    };
    Some(SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: name.into(),
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: sub.into(),
            description: String::new(),
            type_flags: 0,
            creature_type: 7,
            creature_family: CreatureFamily::None,
            creature_rank: rank,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [display, 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

#[allow(clippy::too_many_arguments)]
fn unit_create(guid: u64, entry: u32, display: u32, at: br::Spot, cur: u32, max: u32, level: u32, faction: u32, scale: f32) -> SMSG_UPDATE_OBJECT {
    let g = Guid::new(guid);
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(g)
        .set_object_entry(entry as i32)
        .set_object_scale_x(scale)
        .set_unit_bytes_0(Race::Human, Class::Warrior, Gender::Male, Power::Rage)
        .set_unit_health(cur as i32)
        .set_unit_maxhealth(max as i32)
        .set_unit_level(level as i32)
        .set_unit_factiontemplate(faction as i32)
        .set_unit_flags(0)
        .set_unit_boundingradius(0.389 * scale)
        .set_unit_combatreach(1.5 * scale)
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
        backwards_swimming_speed: 2.5,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 7.0,
        orientation: at.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: at.x, y: at.y, z: at.z },
        running_speed: 7.0,
        swimming_speed: 4.722222,
        timestamp: 0,
        turn_rate: std::f32::consts::PI,
        walking_speed: 2.5,
    };
    SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject {
            guid3: g,
            mask2: UpdateMask::Unit(mask),
            movement2: MovementBlock { update_flag: MovementBlock_UpdateFlag::empty().set_living(living) },
            object_type: ObjectType::Unit,
        }],
    }
}

fn walk_to(c: &Writer, guid: u64, from: br::Spot, to: br::Spot, ms: u32) -> io::Result<()> {
    c.send(&SMSG_MONSTER_MOVE {
        guid: Guid::new(guid),
        unknown: 0,
        spline_point: Vector3d { x: from.x, y: from.y, z: from.z },
        spline_id: (now_ms() / 50) as u32,
        move_type: SMSG_MONSTER_MOVE_MonsterMoveType::Normal,
        spline_flags: SMSG_MONSTER_MOVE_SplineFlag::empty(),
        duration: ms,
        splines: vec![Vector3d { x: to.x, y: to.y, z: to.z }],
    })
}

fn middle(c: &Writer, text: &str) -> io::Result<()> {
    c.send(&SMSG_NOTIFICATION { notification: text.into() })
}

fn boss_says(c: &Writer, name: &str, text: &str) -> io::Result<()> {
    c.send(&SMSG_MESSAGECHAT {
        chat_type: SMSG_MESSAGECHAT_ChatType::MonsterSay { sender1: name.into(), target1: NamedGuid::zero() },
        language: Language::Universal,
        sender: Guid::new(br::BOSS_GUID),
        flags: 0,
        message: text.into(),
        tag: PlayerChatTag::None,
    })
}

/// Everyone of the fight into the game (the player reached its map).
fn spawn(c: &Writer, shared: &Shared, l: &Live) -> io::Result<()> {
    let f = &l.fight;
    let display = crate::creatures::display(f.def.boss).unwrap_or(0);
    let boss_cur = if f.state == State::Cleared { 0 } else { f.boss_hp.min(u32::MAX as u64) as u32 };
    c.send(&unit_create(br::BOSS_GUID, f.def.boss, display, f.boss_at, boss_cur, f.boss_max.min(u32::MAX as u64) as u32, BOSS_LEVEL, 14, f.def.scale))?;
    for m in f.members.iter().filter(|m| m.bot) {
        c.send(&unit_create(m.guid(), MEMBER_ENTRY + m.id as u32, br::MEMBER_DISPLAYS[m.id as usize % 2], m.at, m.hp, m.max, 80, 35, 1.0))?;
    }
    for a in &f.adds {
        c.send(&unit_create(a.guid, a.entry, crate::creatures::display(a.entry).unwrap_or(0), a.at, a.hp, 30_000, 80, 14, 1.0))?;
    }
    let mut cb = shared.combat.lk();
    cb.set_health(br::BOSS_GUID, combat::Kind::Raid, f.boss_max.min(u32::MAX as u64) as u32, boss_cur);
    Ok(())
}

fn despawn_all(c: &Writer, l: &Live) {
    let mut gs = vec![br::BOSS_GUID];
    gs.extend(l.fight.members.iter().filter(|m| m.bot).map(|m| m.guid()));
    gs.extend(l.fight.adds.iter().map(|a| a.guid));
    for g in gs {
        let _ = c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false });
    }
}

fn results(shared: &Shared, l: &Live, ms: u64, cleared: bool) {
    if l.practice {
        return;
    }
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = br::result_line(&l.fight.def.key, l.fight.heroic, &l.class, &l.spec, ms, cleared, at);
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(shared.cfg.den_dir.join(br::RESULTS)) {
        let _ = writeln!(f, "{line}");
    }
}

/// What the fight said, to the player's game.
fn send(c: &Writer, shared: &Shared, l: &mut Live, evs: Vec<Ev>, now: u64) -> io::Result<()> {
    let boss_name = names(l.fight.def.boss).unwrap_or("The boss");
    for e in evs {
        match e {
            Ev::Say(t) => boss_says(c, boss_name, &t)?,
            Ev::Middle(t) => middle(c, &t)?,
            Ev::Warn { at, ms, .. } => c.send_raw(dojo::SMSG_SPELL_START, &dojo::spell_start_body(br::BOSS_GUID, br::RING_SPELL, ms as u32, [at.x, at.y, at.z]))?,
            Ev::Hit { who: 0, amount, .. } => {
                let outs = shared.combat.lk().raid_hit(now, amount);
                sandbox::send_outs(c, shared, 0, PLAYER_GUID, outs)?;
            }
            Ev::Hit { .. } => {}
            Ev::Heal { who: 0, amount } => {
                let outs = shared.combat.lk().raid_heal(amount);
                sandbox::send_outs(c, shared, 0, PLAYER_GUID, outs)?;
            }
            Ev::Heal { .. } => {}
            Ev::Health { guid, cur, max } => {
                if guid >= br::ADD_GUID {
                    shared.combat.lk().set_health(guid, combat::Kind::Raid, max, cur);
                }
                c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(guid, &[(combat::field::HEALTH, cur)]))?;
            }
            Ev::BossHealth { cur, max } => {
                let (cur, max) = (cur.min(u32::MAX as u64) as u32, max.min(u32::MAX as u64) as u32);
                shared.combat.lk().set_health(br::BOSS_GUID, combat::Kind::Raid, max, cur);
                c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(br::BOSS_GUID, &[(combat::field::HEALTH, cur)]))?;
            }
            Ev::Spawn { guid, entry, at, hp } => {
                c.send(&unit_create(guid, entry, crate::creatures::display(entry).unwrap_or(0), at, hp, hp, 80, 14, 1.0))?;
                shared.combat.lk().set_health(guid, combat::Kind::Raid, hp, hp);
            }
            Ev::Gone(g) => c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: true })?,
            Ev::Walk { guid, to, ms } => {
                let from = l.fight.members.iter().find(|m| m.bot && m.guid() == guid).map(|m| m.at).unwrap_or(to);
                walk_to(c, guid, from, to, ms)?;
            }
            Ev::Phase(n) => {
                shared.event("bossrush", "", &format!("phase {n}"));
                if n > 1 {
                    say(c, &format!("Phase {n}."))?;
                }
            }
            Ev::Edge(e) => say(c, &format!("The floor that holds you: {:.0} yards from the middle.", e.r))?,
            Ev::Wipe { ms, why } => {
                shared.event("bossrush", "", &format!("wipe {ms} {why}"));
                results(shared, l, ms, false);
                say(c, &format!("Wiped at {} ({why}). Ready again in a few seconds: hit the boss to try again.", br::clock(ms)))?;
                l.reset_at = now + READY_AGAIN_MS;
            }
            Ev::Clear { ms } => {
                shared.event("bossrush", "", &format!("clear {ms}"));
                results(shared, l, ms, true);
                c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(br::BOSS_GUID, &[(combat::field::HEALTH, 0), (combat::field::DYNAMIC_FLAGS, 0x20)]))?;
                say(c, &format!("Cleared in {}{}! Ready again in a few seconds (hit the boss), or .den bossrush leave.", br::clock(ms), if l.practice { " (practice: not on the board)" } else { "" }))?;
                l.reset_at = now + READY_AGAIN_MS * 2;
            }
        }
    }
    Ok(())
}

/// Back to the start: everyone up, the player on the platform's spot, the boss whole (Try again).
fn ready_again(c: &Writer, shared: &Shared, l: &mut Live) -> io::Result<()> {
    despawn_all(c, l);
    let hp = {
        let mut cb = shared.combat.lk();
        let outs = cb.refill_me();
        let hp = cb.stats.health;
        drop(cb);
        sandbox::send_outs(c, shared, 0, PLAYER_GUID, outs)?;
        hp
    };
    l.again(hp);
    let s = l.fight.def.start;
    c.send_raw(places::MSG_MOVE_TELEPORT_ACK, &places::teleport_body(PLAYER_GUID, (now_ms() & 0xFFFF_FFFF) as u32, movewatch::server_ms(), s.x, s.y, s.z, s.o))?;
    spawn(c, shared, l)?;
    l.synced = l.fight.boss_hp;
    middle(c, "Ready again: hit the boss to begin.")
}

/// Every 50 ms (world.rs dojo_tick): the launcher's ask, the arrival, the fight.
pub(super) fn tick(shared: &Shared) {
    let live = shared.live.lk().clone();
    let Some((sess_id, w, p)) = live else { return };
    let now = now_ms();
    // the launcher's Boss Rush page: <dens>/bossrush.json, each id once
    let ask = {
        let mut r = shared.rush.lk();
        if now >= r.next_ask {
            r.next_ask = now + 2000;
            std::fs::read_to_string(shared.cfg.den_dir.join(br::ASK_FILE)).ok().and_then(|t| br::read_ask(&t)).filter(|a| a.4 != r.asked).inspect(|a| r.asked = a.4.clone())
        } else {
            None
        }
    };
    if let Some((fight, heroic, class, spec, _)) = ask {
        let sess = Sess { id: sess_id, addons: Vec::new(), me: Me::host(shared) };
        let words: Vec<String> = [fight, if heroic { "heroic".into() } else { "normal".into() }, class, spec].into_iter().collect();
        let _ = cmd(&w, shared, &sess, p, &words);
        return;
    }
    let on_map = map_of(shared, &w);
    let mut guard = shared.rush.lk();
    let Some(mut l) = guard.live.take() else {
        guard.cache.clear();
        return;
    };
    guard.cache = unit_list(&l);
    guard.left = false;
    drop(guard);
    let r = (|| -> io::Result<()> {
        if !l.spawned {
            if on_map == Some(l.fight.def.map) {
                l.spawned = true;
                l.synced = l.fight.boss_hp;
                spawn(&w, shared, &l)?;
                say(&w, &format!("{}: light version (not exact). Hit the boss to begin. .den bossrush leave to go home.", l.fight.def.title))?;
            }
            return Ok(());
        }
        if on_map != Some(l.fight.def.map) {
            return Ok(()); // away for now (a loading screen): nothing happens until they are back
        }
        if l.reset_at > 0 {
            if now >= l.reset_at {
                ready_again(&w, shared, &mut l)?;
            }
            return Ok(());
        }
        // what the player took off the boss and the adds since the last beat (combat.rs)
        let (boss_now, adds_now, me) = {
            let cb = shared.combat.lk();
            let adds: Vec<(u64, u32)> = l.fight.adds.iter().map(|a| (a.guid, cb.health_of(a.guid).unwrap_or(a.hp))).collect();
            (cb.health_of(br::BOSS_GUID).map(|h| h as u64), adds, (cb.health, cb.stats.health))
        };
        let mut evs = Vec::new();
        if let Some(b) = boss_now {
            if b < l.synced {
                evs.extend(l.fight.hurt_boss(l.synced - b, now));
            }
        }
        for (g, cur) in adds_now {
            if let Some(a) = l.fight.adds.iter().find(|a| a.guid == g) {
                if cur < a.hp {
                    let d = a.hp - cur;
                    evs.extend(l.fight.hurt_add(g, d));
                }
            }
        }
        l.fight.player_health(me.0, me.1);
        let at = br::Spot { x: p.x, y: p.y, z: p.z, o: p.o };
        evs.extend(l.fight.tick(now, at));
        send(&w, shared, &mut l, evs, now)?;
        l.synced = l.fight.boss_hp;
        Ok(())
    })();
    let _ = r;
    let mut g = shared.rush.lk();
    g.cache = if l.spawned { unit_list(&l) } else { Vec::new() };
    // a command while this beat was worked (leave, a new fight) wins
    if g.live.is_none() && !g.left {
        g.live = Some(l);
    }
    g.left = false;
}

/// `.den bossrush ...`
pub(super) fn cmd(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, words: &[String]) -> io::Result<()> {
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    let list = || br::CATALOG.iter().filter_map(|(k, _)| br::encounter(k)).map(|d| format!("{} ({})", d.key, d.title)).collect::<Vec<_>>().join(", ");
    match w(0) {
        "" | "list" | "help" => say(c, &format!(
            "Boss Rush, light version (our own recreation, not exact): .den bossrush <fight> [normal|heroic] [class spec]. Fights: {}. Also: .den bossrush again, phase <n|end> (practice), status, leave. The exact fights are on the launcher's Boss Rush page.",
            list()
        )),
        "leave" | "stop" | "home" => {
            let old = {
                let mut g = shared.rush.lk();
                g.left = true;
                g.cache.clear();
                g.live.take()
            };
            if let Some(l) = old.as_ref().filter(|l| l.spawned) {
                despawn_all(c, l);
            }
            let outs = shared.combat.lk().refill_me();
            sandbox::send_outs(c, shared, sess.id, sess.me.guid, outs)?;
            if old.is_some() {
                travel(c, shared, sess, home_target(shared))
            } else {
                say(c, "You're not in a Boss Rush fight.")
            }
        }
        "again" | "retry" => {
            let mut g = shared.rush.lk();
            match g.live.as_mut() {
                Some(l) if l.spawned => {
                    l.reset_at = now_ms();
                    drop(g);
                    say(c, "Ready again in a moment.")
                }
                _ => {
                    drop(g);
                    say(c, "You're not in a Boss Rush fight.")
                }
            }
        }
        "status" => {
            let g = shared.rush.lk();
            match g.live.as_ref() {
                Some(l) => {
                    let f = &l.fight;
                    let line = format!("{} ({}): {:?}, phase {}, boss {}%, {} adds.", f.def.title, if f.heroic { "heroic" } else { "normal" }, f.state, f.phase + 1, f.boss_hp * 100 / f.boss_max.max(1), f.adds.len());
                    drop(g);
                    say(c, &line)
                }
                None => {
                    drop(g);
                    say(c, "You're not in a Boss Rush fight.")
                }
            }
        }
        "phase" => {
            let mut g = shared.rush.lk();
            let Some(l) = g.live.as_mut().filter(|l| l.spawned && l.reset_at == 0) else {
                return say(c, "Start a fight first: .den bossrush <fight>.");
            };
            let Some(hp) = br::phase_health(&l.fight, w(1)) else {
                let n = l.fight.def.phases.len();
                return say(c, &format!("This fight has phases 1 to {n}: .den bossrush phase <n|end>."));
            };
            let now = now_ms();
            let mut evs = if l.fight.state == State::Ready { l.fight.pull(now) } else { Vec::new() };
            let cut = l.fight.boss_hp.saturating_sub(hp);
            l.practice = true;
            evs.extend(l.fight.hurt_boss(cut, now));
            let mut lv = g.live.take().expect("there");
            drop(g);
            let r = send(c, shared, &mut lv, evs, now);
            lv.synced = lv.fight.boss_hp;
            shared.rush.lk().live = Some(lv);
            r?;
            say(c, "Practice: the boss's health set for that phase (this run won't go on your board).")
        }
        key => {
            let Some(def) = br::encounter(key) else {
                return say(c, &format!("No light version of '{key}'. Fights: {}.", list()));
            };
            let heroic = words.iter().any(|x| x == "heroic");
            let rest: Vec<&str> = words.iter().skip(1).map(String::as_str).filter(|x| *x != "heroic" && *x != "normal").collect();
            let (class, spec) = (rest.first().copied().unwrap_or(""), rest.get(1).copied().unwrap_or(""));
            if !class.is_empty() {
                sandbox::class_cmd(c, shared, sess, class, &format!("{spec} 80"))?;
            }
            let (hp, kit_class) = {
                let cb = shared.combat.lk();
                (cb.stats.health, cb.kit.as_ref().map(|k| k.class).unwrap_or(0))
            };
            let class_word = if class.is_empty() { crate::classes::CLASSES.iter().find(|c| c.id == kit_class).map(|c| c.word).unwrap_or("-") } else { class };
            let old = shared.rush.lk().live.take();
            if let Some(o) = old.as_ref().filter(|l| l.spawned) {
                despawn_all(c, o);
            }
            let seed = now_ms() | 1;
            let l = Live::new(&def, heroic, class_word, if spec.is_empty() { "-" } else { spec }, hp.max(1), br::role_of(class_word, spec), seed);
            let s = def.start;
            let title = def.title.clone();
            shared.rush.lk().live = Some(l);
            shared.event("bossrush", &sess.me.name, &format!("{key} {}", if heroic { "heroic" } else { "normal" }));
            let _ = p;
            say(c, &format!("Boss Rush, light version (not exact): {title}{}.", if heroic { ", heroic" } else { "" }))?;
            travel(c, shared, sess, places::Target { map: def.map, x: s.x, y: s.y, z: s.z + places::TYPED_LIFT, o: s.o, area: None, title })
        }
    }
}
