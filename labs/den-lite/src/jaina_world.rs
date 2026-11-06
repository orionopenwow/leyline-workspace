//! Jaina's brain in the world (a child module of world.rs, so its hooks there stay a few lines; owner 4 Nov:
//! "upgrade the way Jaina thinks, acts, and what she spawns"). jaina.rs decides, this does it:
//!
//! * what happens reaches her: `apply` (pieces placed and taken away: [`jaina_saw`]), saves, growing, flying,
//!   friends coming and going, her builds done ([`jaina_feed`]);
//! * her thought each tick ([`brain_tick`], from companion_tick): a gesture (SMSG_EMOTE), a stroll to look around,
//!   a line as a toast in the Den bar (she turns to you first), a suggestion, her critters' steps;
//! * her newer words ([`jaina_word`]: "Jaina, quiet", "help", "yes", "no", "tidy up", "a pet", "pets away", "what
//!   did I build?") and "build a <set>" ([`build_or_offer`]: her small sets are a preview to keep; stamps as before);
//! * suggestions: the set's pieces shown to you only, sparkling (the placement aids' glow, gizmo.rs GLOW), never in
//!   the Den until "yes" (then a stamp: saved, one undo); "no", a minute unanswered, or leaving: they go;
//! * critters: units shown to the host (pets.rs bodies), never saved, at most jaina::MAX_CRITTERS;
//! * the Den panel's Jaina row (Dojo.lua): `wuden jaina <talk=..;doing=..;mood=..;offer=..;ask=..;mine=..;...>`.
//!
//! Locks: nothing here holds the companion's lock while it takes the Den's or calls `apply` (which looks at both,
//! with try_lock, so a caller holding either never deadlocks: that change is just not reacted to).

use super::*;
use crate::flight;
use crate::jaina::{self, Act, Question, Sense, Word};
use crate::pets;
use std::f32::consts::{PI, TAU};

/// Something happened that she may react to.
pub(super) fn jaina_feed(shared: &Shared, ev: jaina::Event) {
    shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.feed(ev, now_ms());
}

/// A Den change (world.rs `apply`): the pieces placed (props only, not the room's walls) and taken away.
pub(super) fn jaina_saw(shared: &Shared, ch: &Change) {
    let (made, room) = match shared.den.try_lock() {
        Ok(d) => {
            let made: Vec<(u64, u32)> = ch.created.iter().filter(|o| d.placed(o.guid).is_some()).map(|o| (o.guid, o.entry)).collect();
            let room = made.len() < ch.created.len();
            (made, room)
        }
        Err(_) => {
            let wall = |e: u32| den::LOOKS.iter().any(|l| l.floor == e || l.wall == e);
            let made: Vec<(u64, u32)> = ch.created.iter().filter(|o| !wall(o.entry)).map(|o| (o.guid, o.entry)).collect();
            let room = made.len() < ch.created.len();
            (made, room)
        }
    };
    // a new look or size for the room takes walls away: that isn't you deleting
    let gone: &[u64] = if room { &[] } else { &ch.destroyed };
    if let Ok(mut comp) = shared.companion.try_lock() {
        comp.brain.saw_change(&made, gone, now_ms());
    }
}

/// Her line for the Den panel (Dojo.lua's Jaina row) to the host, when it changed (or `force`).
pub(super) fn jaina_panel(shared: &Shared, force: bool) -> io::Result<()> {
    let line = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        let l = comp.brain.ui_line();
        if !force && comp.panel_sent == l {
            return Ok(());
        }
        comp.panel_sent = l.clone();
        l
    };
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| (l.0, l.1.clone()));
    if let Some((sess, w)) = live {
        bar_data(&w, shared, sess, PLAYER_GUID, "jaina", &line)?;
    }
    Ok(())
}

/// A piece's name for her lines: the catalog's, without its trailing number ("Barrel 01": "Barrel").
pub(super) fn piece_name(entry: u32) -> String {
    den::template(entry)
        .map(|t| t.name.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ').to_string())
        .filter(|n| !n.is_empty() && !companion::branded(n))
        .unwrap_or_else(|| "piece".into())
}

/// Her walk, or her hop (flight.rs hop_path): told she flies, then the arc as a flight.
pub(super) fn step_send(c: &Writer, from: At, to: At, ms: u32, id: u32, via: &[At], hop: bool) -> io::Result<()> {
    if !hop {
        return walk(c, from, to, ms, id, via);
    }
    let mut pts = via.to_vec();
    pts.push(to);
    c.send_raw(flight::SMSG_SPLINE_MOVE_SET_FLYING, &flight::guid_body(companion::GUID))?;
    c.send_raw(flight::SMSG_SPLINE_MOVE_GRAVITY_DISABLE, &flight::guid_body(companion::GUID))?;
    c.send_raw(flight::SMSG_MONSTER_MOVE, &flight::sky_body(companion::GUID, from, &pts, id, flight::SPLINE_FLYING | flight::SPLINE_CATMULLROM, ms))
}

/// A stroll (the walking animation: MoveSplineFlag Walkmode), curved through its waypoints around props.
fn stroll(c: &Writer, from: At, to: At, ms: u32, id: u32, via: &[At]) -> io::Result<()> {
    let mut pts = via.to_vec();
    pts.push(to);
    let flags = companion::SPLINE_WALKMODE | if via.is_empty() { 0 } else { flight::SPLINE_CATMULLROM };
    c.send_raw(flight::SMSG_MONSTER_MOVE, &flight::sky_body(companion::GUID, from, &pts, id, flags, ms))
}

fn emote(c: &Writer, e: u32) -> io::Result<()> {
    c.send_raw(jaina::SMSG_EMOTE, &jaina::emote_body(companion::GUID, e))
}

fn destroy(c: &Writer, guids: &[u64]) -> io::Result<()> {
    for &g in guids {
        c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false })?;
    }
    Ok(())
}

/// An unasked line: she turns to you (when standing), then it is a toast in the Den bar (without the bar: her line
/// in the chat, as notice.rs routes it).
fn jaina_says(shared: &Shared, w: &Writer, p: At, line: &str) -> io::Result<()> {
    let (l, face) = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        let l = companion::clean(&comp.brain.name_in(line, piece_name));
        (l, comp.face_to(p, now_ms()))
    };
    if l.is_empty() || companion::branded(&l) {
        return Ok(());
    }
    if let Some((here, id, o)) = face {
        w.send_raw(dojo::SMSG_MONSTER_MOVE, &dojo::facing_body(companion::GUID, here, id, o))?;
    }
    shared.event("companion", "", "says");
    notify_host(shared, Item::jaina(l))
}

/// Twice a second with her out and following you (companion_tick): her thought (at most one a second) done, her
/// critters' steps, the Den panel's line when it changed.
pub(super) fn brain_tick(shared: &Shared, w: &Writer, p: At, ms: u64) -> io::Result<()> {
    let building = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job.is_some();
    let (mid, reach, blocks) = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let b: Vec<(f32, f32, f32)> = companion_blocks(&d).iter().map(|b| (b.x, b.y, b.r)).collect();
        (At { x: d.spot.x, y: d.spot.y, z: d.spot.z, o: 0.0 }, d.spot.reach(), b)
    };
    let month = den::civil(now()).1;
    let (acts, steps) = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        let s = Sense {
            ms,
            player: p,
            her: comp.pos(ms),
            walking: comp.walking(ms),
            following: comp.out && comp.order == companion::Order::Follow,
            flying: comp.flying,
            building,
            mid,
            reach,
            month,
        };
        let acts = comp.brain.think(&s, &blocks);
        (acts, comp.brain.critter_steps(ms, &blocks))
    };
    for a in acts {
        match a {
            Act::Emote(e) => emote(w, e)?,
            Act::Line(l) => jaina_says(shared, w, p, &l)?,
            Act::Wander(t) => {
                let z = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(t.x, t.y);
                let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
                let step = comp.wander(At { z, ..t }, ms);
                let (id, via) = (comp.moves, comp.via.clone());
                drop(comp);
                if let Some((from, to, d)) = step {
                    stroll(w, from, to, d, id, &via)?;
                    shared.event("companion", "", "explores");
                }
            }
            Act::Offer(key) => {
                if let Some(t) = jaina::theme_by_key(key) {
                    offer(w, shared, p, Some(t), false, false)?;
                }
            }
            Act::Withdraw(gs) => {
                destroy(w, &gs)?;
                shared.event("companion", "", "offer withdrawn");
            }
        }
    }
    if !steps.is_empty() {
        let zs: Vec<f32> = {
            let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
            steps.iter().map(|s| d.floor_at(s.2.x, s.2.y)).collect()
        };
        for ((g, from, to, t), z) in steps.into_iter().zip(zs) {
            w.send_raw(pets::SMSG_MONSTER_MOVE, &pets::move_body(g, [from.x, from.y, from.z], [to.x, to.y, z], (ms & 0xFFFF_FFFF) as u32, t))?;
        }
    }
    jaina_panel(shared, false)
}

// ---------------------------------------------------------------- her sets: preview, keep, send away

/// A set as a stamp document (den.rs `stamp` lays it out and keeps it as one undo).
pub(super) fn theme_doc(t: &jaina::Theme) -> den::DenDoc {
    den::DenDoc {
        format: "leyline-den".into(),
        version: 1,
        kind: Some("stamp".into()),
        name: Some(t.key.into()),
        title: Some(t.title.into()),
        props: t.pieces.iter().map(|&(e, x, y, yaw)| den::Prop { entry: e as u64, x: x as f64, y: y as f64, yaw: yaw as f64, ..Default::default() }).collect(),
        ..Default::default()
    }
}

/// A preview piece's guid: a gameobject, its entry, a counter just below the placement aids' (gizmo.rs AID_BASE).
pub(super) fn preview_guid(entry: u32, i: usize) -> u64 {
    (0xF110u64 << 48) | ((entry as u64 & 0xFF_FFFF) << 24) | (0xFF_FC00 + (i as u64 & 0xFF))
}

/// The set's pieces where `stamp` would put them (settled the same way: on table tops, nothing in another), as
/// objects that aren't props: the preview. Pieces outside the Den are left out, as `stamp` leaves them.
pub(super) fn preview_objects(d: &den::Den, doc: &den::DenDoc, at: At) -> Vec<WorldObject> {
    let settled = crate::stamps::settle(doc);
    let step = PI / 12.0;
    let o = ((at.o / step).round() * step).rem_euclid(TAU);
    let (c, s) = (o.cos(), o.sin());
    settled
        .props
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let t = den::template(e.entry as u32)?;
            let (x, y) = (at.x + e.x as f32 * c - e.y as f32 * s, at.y + e.x as f32 * s + e.y as f32 * c);
            let z = d.floor_at(x, y) + e.z as f32;
            d.inside(x, y, z).then(|| WorldObject {
                guid: preview_guid(t.entry, i),
                entry: t.entry,
                display: t.display,
                kind: t.kind,
                scale: e.scale.map(|v| v.clamp(0.1, 5.0) as f32).unwrap_or(t.size),
                x,
                y,
                z,
                o: (o + (e.yaw as f32).to_radians()).rem_euclid(TAU),
                pitch: 0.0,
                roll: 0.0,
            })
        })
        .collect()
}

/// A set of hers as a sparkling preview in front of you: `theme` None is her pick (in season first when
/// `seasonal`). `asked`: you asked (her answer in her voice); else her own idea (a toast). "Jaina, yes" keeps it.
fn offer(c: &Writer, shared: &Shared, p: At, theme: Option<&'static jaina::Theme>, asked: bool, seasonal: bool) -> io::Result<()> {
    let (ms, month) = (now_ms(), den::civil(now()).1);
    let speak = |text: String| -> io::Result<()> {
        if text.is_empty() {
            Ok(())
        } else if asked {
            monster_say(c, &text)
        } else {
            notify_host(shared, Item::jaina(text))
        }
    };
    if shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job.is_some() {
        return if asked { monster_say(c, dojo::BUSY) } else { Ok(()) };
    }
    let alive: Vec<u64> = shared.den.lock().unwrap_or_else(|e| e.into_inner()).props.iter().map(|x| x.obj.guid).collect();
    let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(comp.brain.question, Some(Question::Offer { .. })) {
        let l = comp.brain.line("offer.busy", &[]);
        drop(comp);
        return speak(l);
    }
    let t = match theme.or_else(|| comp.brain.pick_theme(month, seasonal)).or_else(|| comp.brain.pick_theme(month, false)) {
        Some(t) => t,
        None => {
            let l = comp.brain.line("offer.reject", &[]);
            drop(comp);
            return speak(l);
        }
    };
    // her pieces in this Den: at most MAX_MINE
    comp.brain.forget_gone(|g| alive.contains(&g));
    if comp.brain.mine.len() + t.pieces.len() > jaina::MAX_MINE {
        let l = comp.brain.line("offer.limit", &[]);
        drop(comp);
        return speak(l);
    }
    drop(comp);
    // in front of you (3 yards), moved in as far as it takes to fit the Den, on the ground there
    let ahead = dojo::ahead(p, 3.0);
    let doc = theme_doc(t);
    let made = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let (x, y, too_big) = d.fit_stamp(&doc, ahead.x, ahead.y, p.o);
        let at = At { x, y, z: d.floor_at(x, y), o: p.o };
        if too_big { None } else { Some((preview_objects(&d, &doc, at), at)) }
    };
    let Some((objs, at)) = made.filter(|m| !m.0.is_empty()) else {
        let l = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.line("offer.nospace", &[]);
        return speak(l);
    };
    send_creates(c, shared, &objs)?;
    for o in &objs {
        c.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::gameobject_dynamic(o.guid, gizmo::GLOW))?;
    }
    let line = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        comp.brain.offered(t.key, at, objs.iter().map(|o| o.guid).collect(), asked, ms);
        comp.brain.offer_line(t, month)
    };
    emote(c, jaina::POINT)?;
    shared.event("companion", "", &format!("offer {} {}", t.key, objs.len()));
    speak(line)?;
    jaina_panel(shared, true)
}

/// "Jaina, yes" to a set: the preview goes and the set is put down there for real (a stamp: saved, one undo),
/// hers until you move a piece.
fn keep(c: &Writer, shared: &Shared, theme: &str, at: At, guids: &[u64]) -> io::Result<String> {
    destroy(c, guids)?;
    let Some(t) = jaina::theme_by_key(theme) else { return Ok(String::new()) };
    let ch = shared.den.lock().unwrap_or_else(|e| e.into_inner()).stamp(&theme_doc(t), at.x, at.y, at.z, at.o);
    let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
    if ch.created.is_empty() {
        return Ok(comp.brain.line("offer.nospace", &[]));
    }
    comp.brain.adopt(&ch.created.iter().map(|o| (o.guid, o.x, o.y, o.z)).collect::<Vec<_>>());
    let l = comp.brain.line("offer.accept", &[]);
    drop(comp);
    // her words, not the stamp's line
    apply(c, shared, &Change { message: String::new(), ..ch })?;
    shared.event("companion", "", &format!("kept {theme}"));
    emote(c, jaina::YES)?;
    Ok(l)
}

/// "Jaina, build a <name>": one of her small sets is a preview to keep; a stamp she builds as before (dojo.rs).
pub(super) fn build_or_offer(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, name: &str) -> io::Result<()> {
    match jaina::theme(name) {
        Some(t) => {
            if !shared.companion.lock().unwrap_or_else(|e| e.into_inner()).out {
                return say(c, dojo::NOT_OUT);
            }
            offer(c, shared, p, Some(t), true, false)
        }
        None => jaina_build(c, shared, sess, p, name),
    }
}

// ---------------------------------------------------------------- her newer words

/// "Jaina, quiet" and the others (jaina.rs `word`); `.den jaina <words>` and the Den panel's Jaina row too.
pub(super) fn jaina_word(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, w: Word) -> io::Result<()> {
    let _ = sess;
    let ms = now_ms();
    let out = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        comp.heard_now(now());
        comp.out
    };
    // her answer: in her voice when she is out, else a system line
    let speak = |text: &str| -> io::Result<()> {
        if text.is_empty() {
            Ok(())
        } else if out {
            monster_say(c, text)
        } else {
            say(c, text)
        }
    };
    if !out && !matches!(w, Word::Talk(_) | Word::Memory) {
        return say(c, "Your companion isn't here: .den companion calls her.");
    }
    let line = match w {
        Word::Talk(t) => {
            let l = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.set_talk(t, ms);
            shared.event("companion", "", &format!("talk {}", t.word()));
            l
        }
        Word::Help => return offer(c, shared, p, None, true, false),
        Word::Decorate => return offer(c, shared, p, None, true, true),
        Word::Build(key) => return offer(c, shared, p, jaina::theme_by_key(key), true, false),
        Word::Yes => {
            let q = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.yes();
            match q {
                Some(Question::Offer { theme, at, guids, .. }) => keep(c, shared, theme, at, &guids)?,
                Some(Question::Tidy { guids, .. }) => tidy(c, shared, &guids)?,
                None => shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.line("offer.none", &[]),
            }
        }
        Word::No => {
            let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
            let was_off = comp.brain.offers_off;
            let q = comp.brain.no();
            match q {
                Some(Question::Offer { guids, .. }) => {
                    let key = if comp.brain.offers_off && !was_off { "offer.off" } else { "offer.reject" };
                    let l = comp.brain.line(key, &[]);
                    drop(comp);
                    destroy(c, &guids)?;
                    emote(c, jaina::NO)?;
                    l
                }
                Some(Question::Tidy { .. }) => comp.brain.line("tidy.cancel", &[]),
                None => comp.brain.line("offer.none", &[]),
            }
        }
        Word::Tidy => {
            let props: Vec<(u64, f32, f32, f32, bool)> = shared
                .den
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .props
                .iter()
                .map(|x| (x.obj.guid, x.obj.x, x.obj.y, x.obj.z, x.prop.locked == Some(true)))
                .collect();
            let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
            comp.brain.forget_gone(|g| props.iter().any(|q| q.0 == g));
            let targets = jaina::tidy_targets(&comp.brain.mine, &props);
            // the critters go at once (nothing of yours, nothing saved)
            let critters = comp.brain.critters_away();
            let l = if !targets.is_empty() {
                let n = targets.len().to_string();
                comp.brain.ask_tidy(targets, ms);
                comp.brain.line("tidy.ask", &[("n", &n)])
            } else if !critters.is_empty() {
                comp.brain.line("critter.gone", &[])
            } else {
                comp.brain.line("tidy.none", &[])
            };
            drop(comp);
            destroy(c, &critters)?;
            l
        }
        Word::Critter(which) => {
            let at = {
                let a = dojo::ahead(p, 2.0);
                let z = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(a.x, a.y);
                At { z, o: (p.o + PI).rem_euclid(TAU), ..a }
            };
            let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
            let made = comp.brain.add_critter(which, at, ms);
            match made {
                Ok(cr) => {
                    let name = jaina::critter_name(cr.entry).unwrap_or("critter");
                    let l = comp.brain.line("critter.new", &[("name", name)]);
                    drop(comp);
                    let display = crate::creatures::display(cr.entry).unwrap_or(328);
                    c.send_raw(raw::SMSG_UPDATE_OBJECT, &pets::create_unit_body(cr.guid, [at.x, at.y, at.z, at.o], (ms & 0xFFFF_FFFF) as u32, &jaina::critter_fields(cr.entry, display)))?;
                    shared.event("companion", "", &format!("critter {name}"));
                    l
                }
                Err(key) => comp.brain.line(key, &[]),
            }
        }
        Word::CrittersAway => {
            let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
            let gone = comp.brain.critters_away();
            let l = comp.brain.line(if gone.is_empty() { "critter.none" } else { "critter.gone" }, &[]);
            drop(comp);
            destroy(c, &gone)?;
            l
        }
        Word::Memory => shared.companion.lock().unwrap_or_else(|e| e.into_inner()).brain.memory_line(piece_name),
    };
    speak(&line)?;
    jaina_panel(shared, true)
}

/// "Jaina, yes" to "tidy up?": her pieces you haven't touched go (each one an undo step, as a delete is).
fn tidy(c: &Writer, shared: &Shared, guids: &[u64]) -> io::Result<String> {
    let destroyed: Vec<u64> = {
        let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        guids.iter().filter_map(|g| d.delete(*g).ok()).flat_map(|ch| ch.destroyed).collect()
    };
    // still hers while `apply` looks: her tidy isn't your delete
    apply(c, shared, &Change { destroyed: destroyed.clone(), ..Default::default() })?;
    let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
    comp.brain.forget_gone(|g| !destroyed.contains(&g));
    let n = destroyed.len().to_string();
    shared.event("companion", "", &format!("tidy {n}"));
    Ok(comp.brain.line("tidy.done", &[("n", &n)]))
}

/// CMSG_CREATURE_QUERY for one of her critters: its name, a critter, its model.
pub(super) fn critter_answer(entry: u32) -> Option<SMSG_CREATURE_QUERY_RESPONSE> {
    let name = jaina::critter_name(entry)?;
    let mut title: String = name.split(' ').map(|w| {
        let mut cs = w.chars();
        cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ");
    title.truncate(40);
    Some(SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: title,
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: String::new(),
            description: String::new(),
            type_flags: 0,
            creature_type: 8, // critter
            creature_family: CreatureFamily::None,
            creature_rank: 0,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [crate::creatures::display(entry).unwrap_or(0), 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_set_is_in_the_catalog_and_its_preview_is_where_the_stamp_puts_it() {
        for t in jaina::THEMES {
            for p in t.pieces {
                assert!(den::template(p.0).is_some(), "{}: {} isn't in the catalog", t.key, p.0);
                assert!(!companion::branded(&piece_name(p.0)), "{}", p.0);
            }
        }
        for (e, _, _) in jaina::CRITTERS {
            assert!(crate::creatures::display(*e).is_some(), "critter {e} has a model");
            let a = critter_answer(*e).unwrap().found.unwrap();
            assert!(!companion::branded(&a.name1) && a.name1.chars().next().unwrap().is_uppercase());
        }
        assert!(critter_answer(companion::ENTRY).is_none());
        let mut d = den::Den::new(den::Spot::default());
        let mid = At { x: d.spot.x, y: d.spot.y, z: d.spot.z, o: 0.5 };
        let t = jaina::theme_by_key("lights").unwrap();
        let doc = theme_doc(t);
        let preview = preview_objects(&d, &doc, mid);
        assert_eq!(preview.len(), t.pieces.len());
        assert!(preview.iter().all(|o| d.placed(o.guid).is_none()), "a preview isn't in the Den");
        let ch = d.stamp(&doc, mid.x, mid.y, mid.z, mid.o);
        assert_eq!(ch.created.len(), preview.len());
        for (a, b) in preview.iter().zip(&ch.created) {
            assert_eq!(a.entry, b.entry);
            assert!((a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01 && (a.z - b.z).abs() < 0.01, "{a:?} {b:?}");
            assert!((a.o - b.o).abs() < 0.01, "{a:?} {b:?}");
            assert_ne!(a.guid, b.guid);
        }
        assert_eq!(d.undo().destroyed.len(), preview.len(), "one undo takes the kept set back");
        assert_eq!(piece_name(179967), "Barrel");
        assert_eq!(preview_guid(179967, 2) >> 48, 0xF110);
    }
}
