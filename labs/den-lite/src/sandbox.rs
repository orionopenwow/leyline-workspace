//! The sandbox combat and the class picker in the world (a child module of world.rs, so its hooks there stay a few
//! lines): the game's combat messages to combat.rs and its answers back, the class's fields, spells and bar at login
//! and on `.den class`, the Dojo's practice verbs, the personal bests, and the 50 ms tick.

use super::*;
use crate::classes::{self, Kit};
use crate::combat::{self, Kind, Out, Unit};
use crate::gearsets;
use crate::pets;
use crate::specs;
use crate::spells::{self, Book};
use serde_json::{json, Value};
use std::sync::OnceLock;

/// The game starts moving (forward, back, strafing, a jump): a cast with a cast time stops.
pub(super) const MOVE_STARTS: [u32; 5] = [0x0B5, 0x0B6, 0x0B8, 0x0B9, 0x0BB];
/// `<dens>/dojo-best.json`: the personal bests per class (timed parses, the boss).
pub const BEST_FILE: &str = "dojo-best.json";

fn class_enum(id: u8) -> Class {
    match id {
        2 => Class::Paladin,
        3 => Class::Hunter,
        4 => Class::Rogue,
        5 => Class::Priest,
        6 => Class::DeathKnight,
        7 => Class::Shaman,
        8 => Class::Mage,
        9 => Class::Warlock,
        11 => Class::Druid,
        _ => Class::Warrior,
    }
}

fn power_enum(p: u8) -> Power {
    match p {
        classes::MANA => Power::Mana,
        classes::ENERGY => Power::Energy,
        classes::RUNIC => Power::RunicPower,
        _ => Power::Rage,
    }
}

/// The host's class and level (the character list, name answers): Warrior 1 until one is picked.
pub(super) fn class_of(shared: &Shared) -> (Class, u8) {
    match shared.pick.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(p) => (class_enum(p.class), p.level.clamp(1, 80) as u8),
        None => (Class::Warrior, 1),
    }
}

/// The player's create with the class's fields (player_create, the host's own copy).
pub(super) fn with_class(b: UpdatePlayerBuilder, k: &Kit, g: Gender) -> UpdatePlayerBuilder {
    let st = k.stats;
    let max = st.max_power as i32;
    let b = b
        .set_unit_bytes_0(Race::Human, class_enum(k.class), g, power_enum(st.power))
        .set_unit_health(st.health as i32)
        .set_unit_maxhealth(st.health as i32)
        .set_unit_level(st.level as i32)
        .set_unit_baseattacktime(st.attack_ms as i32)
        .set_unit_mindamage(st.min_dmg)
        .set_unit_maxdamage(st.max_dmg)
        .set_unit_attack_power(st.attack_power as i32)
        // the four extra action bars shown (PLAYER_FIELD_BYTES' third byte): the spec's bars fill them
        .set_player_field_bytes(0, 0, specs::ALL_BARS, 0);
    match st.power {
        classes::MANA => b.set_unit_power1(max).set_unit_maxpower1(max),
        classes::ENERGY => b.set_unit_power4(max).set_unit_maxpower4(max),
        classes::RUNIC => b.set_unit_power7(max).set_unit_maxpower7(max),
        _ => b.set_unit_power2(max).set_unit_maxpower2(max),
    }
}

/// The class's fields as a values update (`.den class` while in the world).
pub fn class_fields(k: &Kit, female: bool) -> Vec<(u16, u32)> {
    let st = k.stats;
    let bytes0 = classes::RACE as u32 | (k.class as u32) << 8 | (female as u32) << 16 | (st.power as u32) << 24;
    vec![
        (combat::field::BYTES_0, bytes0),
        (combat::field::HEALTH, st.health),
        (combat::field::MAXHEALTH, st.health),
        (combat::field::POWER1 + st.power as u16, st.max_power),
        (combat::field::MAXPOWER1 + st.power as u16, st.max_power),
        (combat::field::LEVEL, st.level),
        (combat::field::BASEATTACKTIME, st.attack_ms),
        (combat::field::MINDAMAGE, st.min_dmg.to_bits()),
        (combat::field::MAXDAMAGE, st.max_dmg.to_bits()),
        (combat::field::ATTACK_POWER, st.attack_power),
        (PLAYER_FIELD_BYTES, (specs::ALL_BARS as u32) << 16),
    ]
}

/// PLAYER_FIELD_BYTES (its third byte: the extra action bars shown).
pub const PLAYER_FIELD_BYTES: u16 = 153;

fn empty_book() -> Arc<Book> {
    static EMPTY: OnceLock<Arc<Book>> = OnceLock::new();
    EMPTY.get_or_init(|| Arc::new(Book::default())).clone()
}

/// The tables when a class is picked (Spell.dbc is big: never read for nothing).
fn book_for(shared: &Shared) -> Arc<Book> {
    let picked = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    let mounts = !shared.combat.lock().unwrap_or_else(|e| e.into_inner()).extra.is_empty();
    if picked || mounts { spells::book(&shared.cfg.den_dir) } else { empty_book() }
}

/// `<dens>/den-lite.mounts`: there, every mount is known (`.den mounts`).
pub const MOUNTS_FILE: &str = "den-lite.mounts";

/// The kit for a pick: the spec's spells and talents (the player's own Talent tables), its bars (the mounts on the
/// right bar), the numbers with the gear's tier.
fn make_kit(shared: &Shared, pick: &classes::Pick, mounts: &[u32]) -> Option<Kit> {
    let book = spells::book(&shared.cfg.den_dir);
    let talents = specs::Talents::read(&shared.cfg.den_dir);
    let tier = shared.gear.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|g| g.tier).unwrap_or(0);
    classes::kit_with(&book, &talents, pick, tier, mounts)
}

/// Before the host's Enter World: the class's kit (spells, bar, numbers), full health and power.
pub(super) fn prepare(shared: &Shared) {
    let pick = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mounts = if shared.cfg.den_dir.join(MOUNTS_FILE).exists() { spells::mounts(&spells::book(&shared.cfg.den_dir)) } else { Vec::new() };
    let kit = pick.as_ref().and_then(|p| make_kit(shared, p, &mounts));
    let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
    cb.set_kit(kit);
    cb.extra = mounts;
}

/// After the host's own create (Enter World, a far `.den go`): the talents the spec has (SMSG_TALENTS_INFO) and the
/// weapons as real items (so spells that need one can be cast).
pub(super) fn after_create(c: &Writer, shared: &Shared, me: &Me) -> io::Result<()> {
    if !me.host {
        return Ok(());
    }
    if let Some(k) = kit_for(shared, me) {
        c.send_raw(specs::SMSG_TALENTS_INFO, &specs::talents_info_body(&k.talents, 0))?;
    }
    // a fresh game knows none of the items yet
    SENT_WEAPONS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    send_weapons(c, shared, me.guid)
}

/// The weapon items the game has been sent (their guids), this session.
static SENT_WEAPONS: std::sync::Mutex<Vec<(u8, u64)>> = std::sync::Mutex::new(Vec::new());

/// The weapons worn (main hand, off hand, ranged: what the player's Item.dbc says is a weapon or a shield) as real
/// items: the ones no longer worn destroyed, the new ones created, the player's inventory fields pointing at them.
pub(super) fn send_weapons(c: &Writer, shared: &Shared, me: u64) -> io::Result<()> {
    let items = crate::gear::items(&shared.cfg.den_dir);
    let worn = shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
    let now: Vec<(u8, u64, u32)> = [15u8, 16, 17]
        .iter()
        .filter_map(|&slot| {
            let e = worn.slots.get(&slot).copied().unwrap_or(0);
            let k = items.kinds.get(&e)?;
            (k.0 == 2 || (k.0 == 4 && k.1 == 6)).then(|| (slot, pets::item_guid(((e & 0xF_FFFF) << 4) | slot as u32), e))
        })
        .collect();
    let mut sent = SENT_WEAPONS.lock().unwrap_or_else(|e| e.into_inner());
    for (_, g) in sent.iter().filter(|(_, g)| !now.iter().any(|n| n.1 == *g)) {
        c.send_raw(pets::SMSG_DESTROY_OBJECT, &pets::destroy_body(*g))?;
    }
    for (_, g, e) in now.iter().filter(|n| !sent.iter().any(|x| x.1 == n.1)) {
        c.send_raw(combat::SMSG_UPDATE_OBJECT, &pets::create_item_body(*g, *e, me))?;
    }
    let slots: Vec<(u8, u64)> = [15u8, 16, 17].iter().map(|s| (*s, now.iter().find(|n| n.0 == *s).map(|n| n.1).unwrap_or(0))).collect();
    c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(me, &pets::inv_fields(&slots)))?;
    *sent = now.iter().map(|n| (n.0, n.1)).collect();
    Ok(())
}

/// CMSG_ITEM_QUERY_SINGLE: what the game asks about an item it was given (its class and subclass from the player's
/// Item.dbc; Den Lite's name; the class's weapon damage). Unknown: not answered here.
fn item_query(c: &Writer, shared: &Shared, body: &[u8]) -> io::Result<bool> {
    let Some(entry) = pets::read_item_query(body) else { return Ok(false) };
    let items = crate::gear::items(&shared.cfg.den_dir);
    let (Some(&(display, inv)), Some(&(class, sub))) = (items.by_id.get(&entry), items.kinds.get(&entry)) else { return Ok(false) };
    let st = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).stats;
    c.send_raw(pets::SMSG_ITEM_QUERY_SINGLE_RESPONSE, &pets::item_query_body(entry, class, sub, display, inv, st.min_dmg, st.max_dmg, st.attack_ms))?;
    Ok(true)
}

/// The class's fields, spells, bars and talents to the game at once (a class, spec, loadout or gear change); the
/// pet goes when the class changes.
fn send_kit(c: &Writer, shared: &Shared, sess: &Sess, kit: &Kit, class_changed: bool) -> io::Result<()> {
    let female = sess.me.gender == HeroGender::Female;
    c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(sess.me.guid, &class_fields(kit, female)))?;
    let (known, outs) = {
        let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
        let outs = if class_changed { cb.dismiss() } else { Vec::new() };
        cb.set_kit(Some(kit.clone()));
        (cb.known(), outs)
    };
    send_outs(c, shared, sess.id, sess.me.guid, outs)?;
    c.send_raw(classes::SMSG_INITIAL_SPELLS, &classes::initial_spells_body(&with_place(&known)))?;
    c.send_raw(classes::SMSG_ACTION_BUTTONS, &classes::action_buttons_body(&kit.buttons, 1))?;
    c.send_raw(specs::SMSG_TALENTS_INFO, &specs::talents_info_body(&kit.talents, 0))?;
    loadout_line(c, shared, sess)
}

/// The Dojo panel's loadout line: `wuden loadout class=<word>;spec=<word>;specs=<a,b,c>;bars=<name|->;saved=<a,b>;
/// set=<name|->;tiers=<1,2,..>;mine=<a,b>` (short: an addon message holds 255 bytes).
pub(super) fn loadout_line(c: &Writer, shared: &Shared, sess: &Sess) -> io::Result<()> {
    let pick = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(p) = pick.filter(|p| p.class_def().is_some()) else { return Ok(()) };
    let cd = p.class_def().map(|c| c.word).unwrap_or("-");
    let spec = p.spec_def();
    let specs_of: Vec<&str> = specs::of_class(p.class).iter().map(|s| s.word).collect();
    let bars = p.active.get(&p.key()).cloned().unwrap_or_else(|| "-".into());
    let saved = p.loadout_names().join(",");
    let worn = shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
    let tiers: Vec<String> = match spec {
        Some(sd) => gearsets::available(&gearsets::Sets::read(&shared.cfg.den_dir), &crate::gear::items(&shared.cfg.den_dir), sd).iter().map(|t| t.to_string()).collect(),
        None => Vec::new(),
    };
    let mine: Vec<String> = gearsets::Saved::load(&shared.cfg.den_dir).sets.keys().take(8).cloned().collect();
    let line = format!(
        "class={cd};spec={};specs={};bars={bars};saved={saved};set={};tiers={};mine={}",
        spec.map(|s| s.word).unwrap_or("-"),
        specs_of.join(","),
        if worn.set_name.is_empty() { "-" } else { worn.set_name.as_str() },
        tiers.join(","),
        mine.join(",")
    );
    let line: String = line.chars().take(230).collect();
    bar_data(c, shared, sess.id, sess.me.guid, "loadout", &line)
}

/// The host's kit, when a class is picked.
pub(super) fn kit_for(shared: &Shared, me: &Me) -> Option<Kit> {
    if !me.host {
        return None;
    }
    shared.combat.lock().unwrap_or_else(|e| e.into_inner()).kit.clone()
}

/// The class's spells plus the editor's exact-click spell (gizmo.rs PLACE_SPELL), once.
fn with_place(spells: &[u32]) -> Vec<u32> {
    let mut v = spells.to_vec();
    if !v.contains(&crate::gizmo::PLACE_SPELL) {
        v.push(crate::gizmo::PLACE_SPELL);
    }
    v
}

/// SMSG_INITIAL_SPELLS and SMSG_ACTION_BUTTONS at Enter World: the class's, or none as before.
pub(super) fn initial(c: &Writer, shared: &Shared, me: &Me) -> io::Result<()> {
    let known = if me.host { shared.combat.lock().unwrap_or_else(|e| e.into_inner()).known() } else { Vec::new() };
    match kit_for(shared, me) {
        Some(k) => {
            let book = if known.is_empty() { &k.spells } else { &known };
            c.send_raw(classes::SMSG_INITIAL_SPELLS, &classes::initial_spells_body(&with_place(book)))?;
            c.send_raw(classes::SMSG_ACTION_BUTTONS, &classes::action_buttons_body(&k.buttons, 0))
        }
        None if known.len() > 1 => {
            c.send_raw(classes::SMSG_INITIAL_SPELLS, &classes::initial_spells_body(&with_place(&known)))?;
            c.send(&SMSG_ACTION_BUTTONS::Initial { data: [ActionButton { action: 0, action_type: 0, misc: 0 }; 144] })
        }
        None => {
            // one spell: the editor's exact click (gizmo.rs PLACE_SPELL, never cast)
            c.send_raw(classes::SMSG_INITIAL_SPELLS, &classes::initial_spells_body(&[crate::gizmo::PLACE_SPELL]))?;
            c.send(&SMSG_ACTION_BUTTONS::Initial { data: [ActionButton { action: 0, action_type: 0, misc: 0 }; 144] })
        }
    }
}

/// The looks picked (`.den gear`) on the player's create: every visible slot.
pub(super) fn with_gear(mut b: UpdatePlayerBuilder, g: &crate::gear::Gear) -> UpdatePlayerBuilder {
    for s in 0..crate::gear::SLOTS_N {
        let e = g.slots.get(&s).copied().unwrap_or(0);
        if let Ok(idx) = VisibleItemIndex::try_from(s) {
            b = b.set_player_visible_item(VisibleItem::new(e, [0, 0]), idx);
        }
    }
    b
}

/// `.den gear ...`: any item's look in its slot, at once for everyone in the Den, kept in the Dens folder.
pub(super) fn gear_cmd(c: &Writer, shared: &Shared, sess: &Sess, words: &[String]) -> io::Result<()> {
    use crate::gear::{self, Ask};
    let w: Vec<&str> = words.iter().map(String::as_str).collect();
    let items = gear::items(&shared.cfg.den_dir);
    let ask = match gear::parse(&w, &items) {
        Ok(a) => a,
        Err(e) => return say(c, &e),
    };
    let mut g = shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
    let line = match ask {
        Ask::Show => {
            let worn: Vec<String> = g.slots.iter().filter(|(_, e)| **e != 0).map(|(s, e)| format!("{} {e}", gear::slot_name(*s))).collect();
            return say(c, &format!(
                "You wear: {}. .den gear <item number> puts its look on (several at once too), .den gear <slot> <item|none>, .den gear list <slot>, .den gear reset. Looks: {}.",
                if worn.is_empty() { "nothing".to_string() } else { worn.join(", ") },
                gear::PRESETS.iter().map(|p| p.0).collect::<Vec<_>>().join(", ")
            ));
        }
        Ask::List(slot, page) => {
            let (ids, n) = items.list(slot, page);
            let pages = n.div_ceil(gear::PAGE).max(1);
            let list = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
            bar_data(c, shared, sess.id, sess.me.guid, "gear", &format!("slot={};page={};pages={pages};ids={list}", gear::slot_name(slot), page + 1))?;
            return say(c, &if n == 0 {
                format!("No {} items known (the launcher copies your game's item table into the Dens folder when Your Den starts).", gear::slot_name(slot))
            } else {
                format!("{} items, page {} of {pages}: {}. .den gear <number> puts one on.", gear::slot_name(slot), page + 1, ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(" "))
            });
        }
        Ask::Reset => {
            g = gear::Gear::default();
            "Back in your own outfit.".to_string()
        }
        Ask::Put(v) => {
            let mut said = Vec::new();
            for (s, e) in v {
                g.set(s, e);
                said.push(if e == 0 { format!("{} off", gear::slot_name(s)) } else { format!("{} {e}", gear::slot_name(s)) });
            }
            format!("Your look: {}. (.den gear reset: your own outfit again)", said.join(", "))
        }
    };
    g.save(&shared.cfg.den_dir);
    *shared.gear.lock().unwrap_or_else(|e| e.into_inner()) = Some(g.clone());
    let body = combat::values_body(sess.me.guid, &g.fields());
    c.send_raw(combat::SMSG_UPDATE_OBJECT, &body)?;
    for o in others(shared, sess.id) {
        let _ = o.w.send_raw(combat::SMSG_UPDATE_OBJECT, &body);
    }
    send_weapons(c, shared, sess.me.guid)?;
    shared.event("gear", &sess.me.name, &g.slots.iter().filter(|(_, e)| **e != 0).map(|(s, e)| format!("{s}:{e}")).collect::<Vec<_>>().join(" "));
    say(c, &line)
}

/// The units the player can fight or help now: the Dojo's dummies, and the companion (friendly).
fn units(shared: &Shared) -> Vec<Unit> {
    let mut v: Vec<Unit> = shared
        .dojo
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .dummies
        .iter()
        .map(|d| Unit { guid: d.guid, at: d.at, kind: d.kind, level: dojo::dummy(d.entry).map(|x| x.3 as u32).unwrap_or(80) })
        .collect();
    let comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
    if comp.out {
        v.push(Unit { guid: companion::GUID, at: comp.pos(now_ms()), kind: Kind::Friend, level: 80 });
    }
    v
}

/// A dummy's health, faction, unit flags and size for its create (dummy_create).
pub(super) fn dummy_look(shared: &Shared, d: &dojo::Dummy) -> (u32, u32, u32, f32) {
    let level = dojo::dummy(d.entry).map(|x| x.3 as u32).unwrap_or(80);
    let hp = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).unit_health(&Unit { guid: d.guid, at: d.at, kind: d.kind, level });
    let scale = match d.kind {
        Kind::Boss => 2.0,
        Kind::Marker => dojo::MARKER_SCALE,
        _ => 1.0,
    };
    (hp.cur, hp.max, d.kind.faction(), scale)
}

/// A dummy's rank in the game's creature answer: the boss and the boss-level dummy show the skull (rank 3, boss).
pub(super) fn rank_of(entry: u32) -> u32 {
    match dojo::kind_of(entry) {
        Kind::Boss | Kind::Heavy => 3,
        _ => 0,
    }
}

/// CMSG_CREATURE_QUERY for a pet Den Lite summoned: its name and model.
pub(super) fn pet_answer(entry: u32) -> Option<SMSG_CREATURE_QUERY_RESPONSE> {
    let p = pets::pet_def(entry)?;
    Some(SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: p.name.into(),
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: String::new(),
            description: String::new(),
            type_flags: 0,
            creature_type: if p.family != 0 { 1 } else { 3 }, // a beast, or a demon / undead / elemental alike
            creature_family: CreatureFamily::None,
            creature_rank: 0,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [pets::pet_display(entry), 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    })
}

/// What combat.rs said, sent: messages, lines, the Dojo panel's readout, the bests.
pub(super) fn send_outs(c: &Writer, shared: &Shared, sess: u64, guid: u64, outs: Vec<Out>) -> io::Result<()> {
    for o in outs {
        match o {
            Out::Pkt(op, b) => c.send_raw(op, &b)?,
            Out::Say(t) => say(c, &t)?,
            Out::Middle(t) => c.send(&SMSG_NOTIFICATION { notification: t })?,
            Out::Event(e, d) => shared.event(e, "", &d),
            Out::Dps(l) => bar_data(c, shared, sess, guid, "dps", &l)?,
            Out::Parse { minutes, dps, total, secs } => {
                let (best, old) = record(&shared.cfg.den_dir, &class_word(shared), &format!("parse{minutes}"), dps as f64, true);
                let note = match (best, old) {
                    (true, Some(o)) => format!(" A new best (it was {o:.0})!"),
                    (true, None) => " Your first for this class: your best so far.".into(),
                    (false, Some(o)) => format!(" Your best is {o:.0}."),
                    (false, None) => String::new(),
                };
                shared.event("dojo", "", &format!("parse {minutes} {dps}"));
                say(c, &format!("Parse done: {total} damage in {secs:.0} s, {dps} per second.{note}"))?;
                bar_data(c, shared, sess, guid, "dps", &format!("dps={dps};total={total};secs={secs:.0};dummy={minutes} min parse;done=1"))?;
            }
            Out::Boss { secs, hits, kicked, casts } => {
                let (best, old) = record(&shared.cfg.den_dir, &class_word(shared), "boss", secs as f64, false);
                let note = match (best, old) {
                    (true, Some(o)) => format!(" A new best time (it was {o:.1} s)!"),
                    (true, None) => " Your first time for this class and spec.".into(),
                    (false, Some(o)) => format!(" Your best is {o:.1} s."),
                    (false, None) => String::new(),
                };
                shared.event("dojo", "", &format!("boss down {secs:.1} {hits} {kicked}/{casts}"));
                say(c, &format!(
                    "The practice boss is down in {secs:.1} s; the circles caught you {hits} time{}; you interrupted {kicked} of its {casts} casts.{note} (.den dojo reset: again)",
                    if hits == 1 { "" } else { "s" }
                ))?;
            }
            Out::Demo { secs, total, dps } => {
                let (best, old) = record(&shared.cfg.den_dir, &class_word(shared), "demo", dps as f64, true);
                let note = match (best, old) {
                    (true, Some(o)) => format!(" A new best for a practice run (it was {o:.0})."),
                    _ => String::new(),
                };
                shared.event("demo", "", &format!("{secs:.0} {total} {dps}"));
                say(c, &format!("Practice run done: {total} in {secs:.0} s, {dps} per second.{note}"))?;
            }
        }
    }
    Ok(())
}

/// The bests' key: the class and spec ("mage-fire"), or "none".
fn class_word(shared: &Shared) -> String {
    shared.pick.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|p| p.class_def().is_some()).map(|p| p.key()).unwrap_or_else(|| "none".into())
}

/// The bests (`dojo-best.json`: class -> {"parse1": dps, "parse3", "parse5", "boss": seconds}).
pub fn bests(dir: &std::path::Path) -> Value {
    std::fs::read_to_string(dir.join(BEST_FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// A result kept when it is the class's best (`higher`: more is better): (it was the best, the old best).
pub fn record(dir: &std::path::Path, class: &str, key: &str, value: f64, higher: bool) -> (bool, Option<f64>) {
    let mut all = bests(dir);
    let old = all.get(class).and_then(|c| c.get(key)).and_then(Value::as_f64);
    let better = match old {
        None => true,
        Some(o) => if higher { value > o } else { value < o },
    };
    if better {
        if !all.get(class).is_some_and(Value::is_object) {
            all[class] = json!({});
        }
        all[class][key] = json!((value * 10.0).round() / 10.0);
        let _ = std::fs::create_dir_all(dir);
        companion::write_json(dir, BEST_FILE, &all);
    }
    (better, old)
}

/// The game's combat messages (combat.rs CLIENT_OPS), read before the message crate: true when taken. A start of
/// movement stops a cast (and goes on to the movement code).
pub(super) fn client_op(c: &Writer, shared: &Shared, sess: &Sess, pos: Option<Pos>, op: u32, body: &[u8]) -> io::Result<bool> {
    let Some(p) = pos else { return Ok(false) };
    // the game asks about an item it was given (a weapon made real): its answer
    if op == pets::CMSG_ITEM_QUERY_SINGLE {
        return item_query(c, shared, body);
    }
    // the pet's bar
    if op == pets::CMSG_PET_ACTION {
        if sess.me.host {
            let us = units(shared);
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).pet_action(&us, body);
            send_outs(c, shared, sess.id, sess.me.guid, outs)?;
        }
        return Ok(true);
    }
    if MOVE_STARTS.contains(&op) {
        if sess.me.host {
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).moved();
            send_outs(c, shared, sess.id, sess.me.guid, outs)?;
        }
        return Ok(false);
    }
    if !combat::CLIENT_OPS.contains(&op) {
        return Ok(false);
    }
    // the editor's exact click (gizmo.rs PLACE_SPELL) is the editor's, never a combat cast
    if op == combat::CMSG_CAST_SPELL && combat::read_cast(body).is_some_and(|r| r.spell == crate::gizmo::PLACE_SPELL) {
        return Ok(false);
    }
    // a guest's game: nothing to cast here (the host's Den is the host's sandbox)
    if !sess.me.host {
        if let Some(r) = combat::read_cast(body).filter(|_| op == combat::CMSG_CAST_SPELL) {
            c.send_raw(combat::SMSG_CAST_FAILED, &combat::failed_body(r.count, r.spell, combat::fail::NOT_KNOWN))?;
        }
        return Ok(true);
    }
    let now = now_ms();
    let us = units(shared);
    let outs = match op {
        combat::CMSG_SET_SELECTION => {
            if let Some(g) = combat::read_guid(body) {
                shared.combat.lock().unwrap_or_else(|e| e.into_inner()).select(g);
            }
            Vec::new()
        }
        combat::CMSG_ATTACKSWING => match combat::read_guid(body) {
            Some(g) => shared.combat.lock().unwrap_or_else(|e| e.into_inner()).attack(now, &us, g),
            None => Vec::new(),
        },
        combat::CMSG_ATTACKSTOP => shared.combat.lock().unwrap_or_else(|e| e.into_inner()).stop(),
        combat::CMSG_CAST_SPELL => match combat::read_cast(body) {
            Some(r) => {
                let book = book_for(shared);
                shared.combat.lock().unwrap_or_else(|e| e.into_inner()).cast(now, p, &us, &book, r)
            }
            None => Vec::new(),
        },
        combat::CMSG_CANCEL_CAST | combat::CMSG_CANCEL_CHANNELLING => shared.combat.lock().unwrap_or_else(|e| e.into_inner()).interrupt(combat::fail::INTERRUPTED),
        combat::CMSG_CANCEL_MOUNT_AURA => shared.combat.lock().unwrap_or_else(|e| e.into_inner()).dismount(),
        combat::CMSG_CANCEL_AURA => match body.get(..4) {
            Some(b) => shared.combat.lock().unwrap_or_else(|e| e.into_inner()).cancel_aura(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            None => Vec::new(),
        },
        _ => {
            // CMSG_SET_ACTION_BUTTON: the player's bar, kept per class
            if let Some((b, a)) = classes::read_set_button(body) {
                let mut g = shared.pick.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(pk) = g.as_mut() {
                    pk.set_button(b, a);
                    pk.save(&shared.cfg.den_dir);
                }
            }
            Vec::new()
        }
    };
    send_outs(c, shared, sess.id, sess.me.guid, outs)?;
    Ok(true)
}

// When the moving dummies take their next step: one clock per Den Lite (its own dojo_ticker thread).
thread_local!(static NEXT_STEP: std::cell::Cell<u64> = const { std::cell::Cell::new(0) });

/// Every 50 ms (dojo_ticker): the moving dummies walk; the fight's tick (casts due, swings, ticks, regen, the
/// dummies' own doings) to the host in the world.
pub(super) fn tick(shared: &Shared) {
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some((sess, w, p)) = live else { return };
    let now = now_ms();
    if now >= NEXT_STEP.with(|n| n.get()) {
        NEXT_STEP.with(|n| n.set(now + dojo::MOVER_STEP_MS));
        let steps = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).mover_steps();
        for (guid, from, to) in steps {
            for pl in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                let _ = pl.w.send(&SMSG_MONSTER_MOVE {
                    guid: Guid::new(guid),
                    unknown: 0,
                    spline_point: Vector3d { x: from.x, y: from.y, z: from.z },
                    spline_id: (now / 100) as u32,
                    move_type: SMSG_MONSTER_MOVE_MonsterMoveType::Normal,
                    spline_flags: SMSG_MONSTER_MOVE_SplineFlag::empty(),
                    duration: dojo::MOVER_STEP_MS as u32,
                    splines: vec![Vector3d { x: to.x, y: to.y, z: to.z }],
                });
            }
        }
    }
    let us = units(shared);
    let book = book_for(shared);
    let outs = {
        let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
        // dummies taken down are forgotten
        let gone: Vec<u64> = cb.hp.keys().copied().filter(|g| !us.iter().any(|u| u.guid == *g)).collect();
        for g in gone {
            cb.forget(g);
        }
        cb.tick(now, p, &us, &book)
    };
    if !outs.is_empty() {
        let _ = send_outs(&w, shared, sess, PLAYER_GUID, outs);
    }
}

/// The Dojo panel line's sandbox part: `;combat=1;class=<word>;level=<n>;fight=0|1;parse=<minutes|->;best=<dps|->`.
pub(super) fn line_extra(shared: &Shared) -> String {
    let (class, level) = match shared.pick.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(p) => (p.class_def().map(|c| c.word).unwrap_or("-").to_string(), p.level),
        None => ("-".to_string(), 1),
    };
    let (fight, parse) = {
        let c = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
        (c.fight, c.parse.as_ref().map(|p| p.minutes.to_string()).unwrap_or_else(|| "-".into()))
    };
    let god = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).god;
    let best = bests(&shared.cfg.den_dir).get(class_word(shared).as_str()).and_then(|c| c.get("boss")).and_then(Value::as_f64).map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into());
    format!(";combat=1;class={class};level={level};fight={};parse={parse};best={best};god={}", fight as u8, god as u8)
}

/// How many of the 144 buttons have something on them.
fn filled(k: &Kit) -> usize {
    k.buttons.iter().filter(|b| **b != 0).count()
}

/// `.den class [<class> [spec] [level]]`: the class picker (the spec and the level in any order).
pub(super) fn class_cmd(c: &Writer, shared: &Shared, sess: &Sess, a: &str, rest: &str) -> io::Result<()> {
    let list = classes::list_line(shared.pick.lock().unwrap_or_else(|e| e.into_inner()).as_ref());
    if a.is_empty() || a == "list" || a == "help" {
        return say(c, &list);
    }
    let Some(cd) = classes::by_word(a) else { return say(c, &format!("No class called '{a}'. {list}")) };
    let mut level_word = "";
    let mut spec_word = "";
    for w in rest.split_whitespace() {
        if w.chars().all(|ch| ch.is_ascii_digit()) {
            level_word = w;
        } else if spec_word.is_empty() {
            spec_word = w;
        }
    }
    let level = match classes::level_arg(level_word) {
        Ok(l) => l,
        Err(e) => return say(c, &e),
    };
    if !spec_word.is_empty() && specs::by_word(cd.id, spec_word).is_none() {
        let names: Vec<&str> = specs::of_class(cd.id).iter().map(|s| s.word).collect();
        return say(c, &format!("A {} has no spec called '{spec_word}': {}.", cd.name, names.join(", ")));
    }
    let pick = {
        let mut g = shared.pick.lock().unwrap_or_else(|e| e.into_inner());
        let mut pk = g.clone().unwrap_or_default();
        (pk.class, pk.level) = (cd.id, level);
        if !spec_word.is_empty() {
            pk.set_spec(spec_word);
        }
        pk.save(&shared.cfg.den_dir);
        *g = Some(pk.clone());
        pk
    };
    let mounts = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).extra.clone();
    let book = spells::book(&shared.cfg.den_dir);
    let Some(kit) = make_kit(shared, &pick, &mounts) else { return say(c, "That class didn't take.") };
    // in the world now: the fields, the spells, every bar and the talents at once (the class's own frames: at the
    // next login)
    send_kit(c, shared, sess, &kit, true)?;
    let n = kit.spells.len();
    let spec = kit.spec_def().map(|s| s.name).unwrap_or("");
    shared.event("class", &sess.me.name, &format!("{} {level} {n} {} {} {}", cd.word, kit.spec, filled(&kit), kit.talents.iter().map(|t| t.1 as u32).sum::<u32>()));
    let tables = if book.spells.is_empty() {
        " Your game's spell tables aren't in the Dens folder yet: start Your Den from the launcher (it copies them from your game), then .den class again."
    } else {
        ""
    };
    let talents = if kit.talents.is_empty() { String::new() } else { format!(", {} talent points", kit.talents.iter().map(|t| t.1 as u32).sum::<u32>()) };
    say(c, &format!(
        "You are a level {level} {spec} {} now: {n} spells{talents}, every bar filled ({} buttons).{tables} .den spec: another spec; .den bars save <name>: keep this bar. Log out and back in for the class's own frames (your class is kept).",
        cd.name,
        filled(&kit)
    ))
}

/// `.den spec [<spec>]`: the class's specs, or that spec (its talents, spellbook and bars at once).
pub(super) fn spec_cmd(c: &Writer, shared: &Shared, sess: &Sess, a: &str) -> io::Result<()> {
    let Some(mut pick) = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|p| p.class_def().is_some()) else {
        return say(c, "Pick a class first: .den class mage (or the Dojo panel's class picker).");
    };
    let cd = pick.class_def().unwrap_or(&classes::CLASSES[0]);
    let names: Vec<String> = specs::of_class(cd.id).iter().map(|s| format!("{} ({})", s.word, s.role.word())).collect();
    if a.is_empty() || a == "list" {
        let now = pick.spec_def().map(|s| s.name).unwrap_or("-");
        return say(c, &format!("You are {now}. {}'s specs: {}. .den spec <name>.", cd.name, names.join(", ")));
    }
    let Some(sd) = pick.set_spec(a) else { return say(c, &format!("A {} has no spec called '{a}': {}.", cd.name, names.join(", "))) };
    pick.save(&shared.cfg.den_dir);
    *shared.pick.lock().unwrap_or_else(|e| e.into_inner()) = Some(pick.clone());
    let mounts = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).extra.clone();
    let Some(kit) = make_kit(shared, &pick, &mounts) else { return say(c, "That spec didn't take.") };
    send_kit(c, shared, sess, &kit, false)?;
    let points: u32 = kit.talents.iter().map(|t| t.1 as u32).sum();
    shared.event("spec", &sess.me.name, &format!("{} {} {} {} {points}", cd.word, sd.word, kit.spells.len(), filled(&kit)));
    say(c, &format!(
        "{} {} now: {} spells, {}, every bar filled ({} buttons). .den bars save <name> keeps a bar; .den bars reset: this layout again.",
        sd.name,
        cd.name,
        kit.spells.len(),
        if points > 0 { format!("{points} talent points") } else { "no talent tables yet (the launcher copies them when Your Den starts)".to_string() },
        filled(&kit)
    ))
}

/// `.den bars [list|save <name>|use <name>|reset|delete <name>]`: named bar loadouts for the class and spec.
pub(super) fn bars_cmd(c: &Writer, shared: &Shared, sess: &Sess, words: &[String]) -> io::Result<()> {
    let Some(mut pick) = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|p| p.class_def().is_some()) else {
        return say(c, "Pick a class first: .den class mage.");
    };
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    let name = words.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
    let mounts = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).extra.clone();
    let line = match w(0) {
        "" | "list" => {
            let n = pick.loadout_names();
            return say(c, &if n.is_empty() {
                "No saved bars for this spec yet: .den bars save <name> keeps the bar as it is now.".to_string()
            } else {
                format!("Your bars for this spec: {}. .den bars use <name>, .den bars reset.", n.join(", "))
            });
        }
        "save" => {
            let Some(kit) = make_kit(shared, &pick, &mounts) else { return Ok(()) };
            match pick.save_loadout(&name, &kit.buttons) {
                Ok(n) => format!("Saved your bar as '{n}'."),
                Err(e) => return say(c, &e),
            }
        }
        "use" | "load" | "switch" => match pick.use_loadout(&name) {
            Ok(n) => format!("Your '{n}' bar is on."),
            Err(e) => return say(c, &e),
        },
        "reset" | "default" => {
            pick.reset_bars();
            "Your spec's own layout again.".to_string()
        }
        "delete" | "remove" => {
            if !pick.delete_loadout(&name) {
                return say(c, &format!("No saved bar called '{}'.", name.trim()));
            }
            format!("'{}' deleted.", name.trim())
        }
        other => match pick.use_loadout(&words.join(" ")) {
            Ok(n) => format!("Your '{n}' bar is on."),
            Err(_) => return say(c, &format!("Usage: .den bars [list|save <name>|use <name>|reset|delete <name>] ('{other}'?)")),
        },
    };
    pick.save(&shared.cfg.den_dir);
    *shared.pick.lock().unwrap_or_else(|e| e.into_inner()) = Some(pick.clone());
    if let Some(kit) = make_kit(shared, &pick, &mounts) {
        c.send_raw(classes::SMSG_ACTION_BUTTONS, &classes::action_buttons_body(&kit.buttons, 1))?;
        shared.combat.lock().unwrap_or_else(|e| e.into_inner()).kit = Some(kit);
    }
    shared.event("bars", &sess.me.name, &format!("{} {}", w(0), name.trim()));
    loadout_line(c, shared, sess)?;
    say(c, &line)
}

/// `.den gearset [list|t1-t10|<saved name>|save <name>|delete <name>|off]`: a whole look at once (the class and
/// spec's raid tier, or one of yours), for everyone in the Den at once; its tier in the sandbox's numbers.
pub(super) fn gearset_cmd(c: &Writer, shared: &Shared, sess: &Sess, words: &[String]) -> io::Result<()> {
    let w: Vec<&str> = words.iter().map(String::as_str).collect();
    let pick = shared.pick.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let spec = pick.as_ref().and_then(|p| p.spec_def());
    let dir = shared.cfg.den_dir.clone();
    let items = crate::gear::items(&dir);
    let mut saved = gearsets::Saved::load(&dir);
    let mut worn = shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
    let line = match w.as_slice() {
        [] | ["list"] => {
            let tiers = spec.map(|sd| gearsets::available(&gearsets::Sets::read(&dir), &items, sd)).unwrap_or_default();
            let mine: Vec<&String> = saved.sets.keys().collect();
            return say(c, &format!(
                "Gear sets{}: {}. Yours: {}. .den gearset t7 puts one on; .den gearset save <name>; .den gearset off.",
                spec.map(|s| format!(" for {}", s.name)).unwrap_or_default(),
                if tiers.is_empty() { "none (pick a class, and the launcher copies your game's item tables when Your Den starts)".to_string() } else { tiers.iter().map(|t| format!("t{t}")).collect::<Vec<_>>().join(" ") },
                if mine.is_empty() { "none".to_string() } else { mine.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ") }
            ));
        }
        ["save", ..] => match saved.put(&w[1..].join(" "), &worn.slots, worn.tier) {
            Ok(n) => {
                saved.save(&dir);
                worn.set_name = n.clone();
                format!("Saved what you wear as '{n}'.")
            }
            Err(e) => return say(c, &e),
        },
        ["delete", ..] | ["remove", ..] => {
            let n = classes::loadout_name(&w[1..].join(" ")).unwrap_or_default();
            if saved.sets.remove(&n).is_none() {
                return say(c, &format!("No gear set called '{n}'."));
            }
            saved.save(&dir);
            format!("'{n}' deleted.")
        }
        ["off"] | ["reset"] | ["none"] => {
            worn = crate::gear::Gear::default();
            "Back in your own outfit.".to_string()
        }
        words => {
            if let Some(t) = gearsets::tier_word(words) {
                let Some(sd) = spec else { return say(c, "Pick a class first: .den class mage, then .den gearset t7.") };
                match gearsets::tier_look(&gearsets::Sets::read(&dir), &items, sd, t) {
                    Some((name, look)) => {
                        worn = crate::gear::Gear::default();
                        for s in 0..crate::gear::SLOTS_N {
                            worn.slots.insert(s, 0);
                        }
                        worn.slots.insert(3, crate::gear::OUTFIT[0].1);
                        for (s, e) in look {
                            worn.set(s, e);
                        }
                        (worn.set_name, worn.tier) = (format!("t{t}"), t);
                        format!("Tier {t}: {name}, with its era's weapons. Your numbers are a tier {t} {}'s now.", sd.name)
                    }
                    None => return say(c, &format!("Tier {t} isn't in your game's item tables for {} (.den gearset lists what is).", sd.name)),
                }
            } else {
                let n = classes::loadout_name(&words.join(" ")).unwrap_or_default();
                match saved.sets.get(&n).cloned() {
                    Some((slots, tier)) => {
                        worn.slots = slots;
                        (worn.set_name, worn.tier) = (n.clone(), tier);
                        format!("Your '{n}' set is on.")
                    }
                    None => return say(c, &format!("No gear set called '{n}': .den gearset lists them.")),
                }
            }
        }
    };
    worn.save(&dir);
    *shared.gear.lock().unwrap_or_else(|e| e.into_inner()) = Some(worn.clone());
    let body = combat::values_body(sess.me.guid, &worn.fields());
    c.send_raw(combat::SMSG_UPDATE_OBJECT, &body)?;
    for o in others(shared, sess.id) {
        let _ = o.w.send_raw(combat::SMSG_UPDATE_OBJECT, &body);
    }
    send_weapons(c, shared, sess.me.guid)?;
    // the tier in the numbers
    if let Some(p) = pick.as_ref() {
        let mounts = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).extra.clone();
        if let Some(kit) = make_kit(shared, p, &mounts) {
            let female = sess.me.gender == HeroGender::Female;
            c.send_raw(combat::SMSG_UPDATE_OBJECT, &combat::values_body(sess.me.guid, &class_fields(&kit, female)))?;
            shared.combat.lock().unwrap_or_else(|e| e.into_inner()).set_kit(Some(kit));
        }
    }
    let slots = worn.slots.values().filter(|e| **e != 0).count();
    shared.event("gearset", &sess.me.name, &format!("{} {} {slots}", if worn.set_name.is_empty() { "-" } else { worn.set_name.as_str() }, worn.tier));
    loadout_line(c, shared, sess)?;
    say(c, &line)
}

/// `.den mounts [on|off|list [page]]`: every mount the player's own tables have, known at once (in the spellbook;
/// cast one to ride, again or right-click its buff to get off; flying ones fly with `.den fly on`); a page of them.
pub(super) fn mounts_cmd(c: &Writer, shared: &Shared, sess: &Sess, a: &str, b: &str) -> io::Result<()> {
    let dir = &shared.cfg.den_dir;
    let book = spells::book(dir);
    let all = spells::mounts(&book);
    if a == "list" {
        let page = b.parse::<usize>().unwrap_or(1).max(1) - 1;
        let per = 15;
        let pages = all.len().div_ceil(per).max(1);
        let ids: Vec<u32> = all.iter().copied().skip(page * per).take(per).collect();
        bar_data(c, shared, sess.id, sess.me.guid, "mounts", &format!("page={};pages={pages};ids={}", page + 1, ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")))?;
        let names: Vec<String> = ids.iter().filter_map(|i| book.spell(*i)).map(|s| s.name.clone()).collect();
        return say(c, &if names.is_empty() { "No mounts known yet: .den mounts".to_string() } else { format!("Mounts, page {} of {pages}: {}.", page + 1, names.join(", ")) });
    }
    let on = a != "off";
    if on && all.is_empty() {
        return say(c, "Your game's spell tables aren't in the Dens folder yet (the launcher copies them when Your Den starts): no mounts to learn.");
    }
    if on {
        let _ = std::fs::write(dir.join(MOUNTS_FILE), "on\n");
    } else {
        let _ = std::fs::remove_file(dir.join(MOUNTS_FILE));
    }
    let (known, outs) = {
        let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
        let outs = if on { Vec::new() } else { cb.dismount() };
        cb.extra = if on { all.clone() } else { Vec::new() };
        (cb.known(), outs)
    };
    send_outs(c, shared, sess.id, sess.me.guid, outs)?;
    c.send_raw(classes::SMSG_INITIAL_SPELLS, &classes::initial_spells_body(&with_place(&known)))?;
    shared.event("mounts", &sess.me.name, &format!("{} {}", if on { "on" } else { "off" }, all.len()));
    say(c, &if on {
        format!("{} mounts in your spellbook (General). Cast one to ride, again to get off; flying ones fly with .den fly on. .den mounts list: their names. If they don't show, log out and in.", all.len())
    } else {
        "The mounts are put away.".to_string()
    })
}

/// The sandbox's lines in `.den dojo help` (world.rs says the Dojo's own after them).
const HELP: &[&str] = &[
    ".den class <class> [spec] [level]: any class and spec, its spellbook, talents and every bar at once; .den spec <spec>: another spec",
    ".den bars save <name> | use <name> | reset | list: your bar loadouts per spec; .den gearset t1-t10 | save <name> | <name> | off: whole looks",
    ".den dojo levels | heavy | light | group | range: dummies three levels around you, the healing range, distance markers",
    ".den dojo demo [seconds]: your main bar cast for you (a practice run); .den dojo invulnerable on|off; .den dojo meter reset",
];

/// The Dojo's sandbox verbs (`.den dojo <verb>`); None: not one of them.
pub(super) fn dojo_verb(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, args: &[String]) -> Option<io::Result<()>> {
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let r = match a(0) {
        "help" | "?" | "commands" => {
            for l in HELP {
                let _ = say(c, l);
            }
            return None;
        }
        // the panel asks for its lines: the loadout's too (then the Dojo's own, from world.rs)
        "status" => {
            let _ = loadout_line(c, shared, sess);
            return None;
        }
        "levels" | "level" => {
            kind_up(shared, p, Kind::Light);
            kind_up(shared, p, Kind::Heavy);
            say(c, "Two more dummies: one three levels below you (no armour), one at a boss's level (heavy armour: misses, dodges and glancing blows count). Your own level's never miss.")
        }
        w if Kind::from_word(w).is_some_and(|k| k != Kind::Target) => {
            let k = Kind::from_word(w).unwrap_or_default();
            kind_up(shared, p, k);
            say(c, kind_line(k))
        }
        "spec" | "specs" => spec_cmd(c, shared, sess, &rest.join(" ")),
        "bars" | "bar" | "loadout" | "loadouts" => bars_cmd(c, shared, sess, &rest),
        "gearset" | "gearsets" | "set" | "sets" => gearset_cmd(c, shared, sess, &rest),
        "invulnerable" | "god" | "immortal" | "unhurt" => {
            let on = dojo::on_off(a(1)).unwrap_or_else(|| !shared.combat.lock().unwrap_or_else(|e| e.into_inner()).god);
            shared.combat.lock().unwrap_or_else(|e| e.into_inner()).god = on;
            shared.event("dojo", &sess.me.name, if on { "invulnerable on" } else { "invulnerable off" });
            say(c, if on { "Nothing hurts you now (.den dojo invulnerable off: as before)." } else { "You can be hurt again (you still never fall)." })
        }
        "meter" if a(1) == "reset" => {
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).reset_meters();
            send_outs(c, shared, sess.id, sess.me.guid, outs).and_then(|_| say(c, "The meters start again."))
        }
        "demo" | "practice" | "run" => {
            // `.den dojo demo [seconds] [kind]`: at your target, else the nearest dummy (of that kind: "boss")
            let secs = a(1).parse::<u32>().unwrap_or(30).clamp(5, 300);
            let kind = Kind::from_word(a(2)).or_else(|| Kind::from_word(a(1)));
            let us = units(shared);
            let target = {
                let cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
                let sel = cb.selection;
                if kind.is_none() && us.iter().any(|u| u.guid == sel && u.kind != Kind::Friend && u.kind != Kind::Marker) {
                    Some(sel)
                } else {
                    us.iter()
                        .filter(|u| match kind {
                            Some(k) => u.kind == k,
                            None => u.kind.hostile(),
                        })
                        .min_by(|x, y| combat::dist(x.at, p).total_cmp(&combat::dist(y.at, p)))
                        .map(|u| u.guid)
                }
            };
            match target {
                Some(t) => {
                    shared.combat.lock().unwrap_or_else(|e| e.into_inner()).start_demo(now_ms(), t, secs);
                    shared.event("dojo", &sess.me.name, &format!("demo {secs}"));
                    say(c, &format!("A {secs} second practice run: your main bar, cast for you at the dummy. Watch the numbers."))
                }
                None => say(c, "Put a dummy up first (.den dojo dummies), or target one."),
            }
        }
        "fight" => match dojo::on_off(a(1)).or_else(|| a(1).is_empty().then(|| !shared.combat.lock().unwrap_or_else(|e| e.into_inner()).fight)) {
            Some(on) => {
                shared.combat.lock().unwrap_or_else(|e| e.into_inner()).fight = on;
                say(c, if on { "The dummies hit back now (lightly; you never fall). .den dojo fight off: they stand still." } else { "The dummies stand still again." })
            }
            None => say(c, "Usage: .den dojo fight on|off"),
        },
        "parse" => match a(1).trim_end_matches('m').parse::<u32>() {
            Ok(m) if [1, 3, 5].contains(&m) => {
                shared.combat.lock().unwrap_or_else(|e| e.into_inner()).start_parse(m);
                say(c, &format!("A {m} minute parse starts with your first hit on a dummy. Your best per class is kept."))
            }
            _ => say(c, "Parses are 1, 3 or 5 minutes: .den dojo parse 3"),
        },
        "reset" => {
            let outs = {
                let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
                let mut o = cb.reset_units();
                o.extend(cb.clear_cooldowns());
                o.extend(cb.refill());
                o
            };
            send_outs(c, shared, sess.id, sess.me.guid, outs).and_then(|_| say(c, "Reset: every dummy full again (the boss back up), your cooldowns ready, your health and power full."))
        }
        "refill" | "mana" | "energy" => {
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).refill();
            send_outs(c, shared, sess.id, sess.me.guid, outs).and_then(|_| say(c, "Health and power full."))
        }
        "cooldowns" | "cd" | "cds" => {
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).clear_cooldowns();
            let n = outs.len();
            send_outs(c, shared, sess.id, sess.me.guid, outs).and_then(|_| say(c, &format!("{n} cooldown{} cleared.", if n == 1 { "" } else { "s" })))
        }
        "best" | "bests" => {
            let all = bests(&shared.cfg.den_dir);
            let mut parts = Vec::new();
            if let Some(m) = all.as_object() {
                for (class, v) in m {
                    let get = |k: &str| v.get(k).and_then(Value::as_f64);
                    let mut bits = Vec::new();
                    for m in [1, 3, 5] {
                        if let Some(d) = get(&format!("parse{m}")) {
                            bits.push(format!("{m} min {d:.0}"));
                        }
                    }
                    if let Some(s) = get("boss") {
                        bits.push(format!("boss {s:.1} s"));
                    }
                    if !bits.is_empty() {
                        parts.push(format!("{class}: {}", bits.join(", ")));
                    }
                }
            }
            say(c, &if parts.is_empty() { "No bests yet: .den dojo parse 1, or .den dojo boss.".to_string() } else { format!("Your bests: {}.", parts.join("; ")) })
        }
        "dps" | "meter" => {
            let m = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).meter.clone();
            say(c, &if m.total == 0 { "Hit a dummy: your numbers show here and in the Dojo panel.".to_string() } else { format!("Now: {} in {:.1} s, {} per second.", m.total, m.secs(), m.per_sec()) })
        }
        "class" => class_cmd(c, shared, sess, a(1), &args.iter().skip(2).cloned().collect::<Vec<_>>().join(" ")),
        "pet" if a(1) == "off" || a(1) == "dismiss" => {
            let outs = shared.combat.lock().unwrap_or_else(|e| e.into_inner()).dismiss();
            send_outs(c, shared, sess.id, sess.me.guid, outs).and_then(|_| say(c, "Your pet is sent away."))
        }
        "mounts" | "mount" => mounts_cmd(c, shared, sess, a(1), a(2)),
        "gear" | "looks" => gear_cmd(c, shared, sess, &args[1..]),
        _ => return None,
    };
    Some(r)
}

fn kind_line(k: Kind) -> &'static str {
    match k {
        Kind::Pack => "A pack of five dummies: try your area spells on them.",
        Kind::Mover => "A moving dummy walks a circle: keep up with it.",
        Kind::Healer => "A wounded dummy keeps losing health: heal it (target it, then your heals). Your healing per second shows.",
        Kind::Tank => "A sparring dummy hits back once you hit it (lightly; you never fall).",
        Kind::Boss => "The practice boss: hit it and it marks the ground under you every few seconds (move out before it lands) and casts at you (interrupt it). Your best time per class and spec is kept.",
        Kind::Light => "A dummy three levels below you, no armour.",
        Kind::Heavy => "A boss-level dummy (heavy armour): misses, dodges and glancing blows count against it.",
        Kind::Group => "The healing range: five friends losing health, the first faster, now and then a big hit on one. Your healing per second shows.",
        Kind::Marker => "The range: a dummy 40 yards ahead, markers at 5, 10, 20 and 30 yards from it. Stand at one and try what reaches.",
        _ => "Training dummies up.",
    }
}

/// The dummies of a kind put up for everyone in the world (the old ones of that kind down).
fn kind_up(shared: &Shared, p: Pos, kind: Kind) {
    let z = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(p.x, p.y);
    let (gone, made) = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).place_kind(p, kind, z);
    {
        let mut cb = shared.combat.lock().unwrap_or_else(|e| e.into_inner());
        for g in &gone {
            cb.forget(*g);
        }
    }
    for pl in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        for &g in &gone {
            let _ = pl.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false });
        }
        for m in &made {
            let _ = pl.w.send(&dummy_create(shared, m));
        }
    }
    shared.event("dojo", "", &format!("{} {}", kind.word(), made.len()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bests_are_kept_per_class_and_only_when_better() {
        let dir = std::env::temp_dir().join(format!("den-lite-bests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(record(&dir, "mage", "parse1", 1200.0, true), (true, None));
        assert_eq!(record(&dir, "mage", "parse1", 1100.0, true), (false, Some(1200.0)));
        assert_eq!(record(&dir, "mage", "parse1", 1300.0, true), (true, Some(1200.0)));
        assert_eq!(record(&dir, "mage", "boss", 90.0, false), (true, None));
        assert_eq!(record(&dir, "mage", "boss", 95.0, false), (false, Some(90.0)));
        assert_eq!(record(&dir, "rogue", "boss", 80.0, false), (true, None));
        let all = bests(&dir);
        assert_eq!(all["mage"]["parse1"].as_f64(), Some(1300.0));
        assert_eq!(all["rogue"]["boss"].as_f64(), Some(80.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_class_fields_say_class_power_and_level() {
        let k = Kit::plain(8, classes::stats(classes::by_word("mage").unwrap(), 80));
        let f = class_fields(&k, true);
        assert_eq!(f[0], (combat::field::BYTES_0, 1 | 8 << 8 | 1 << 16));
        assert!(f.contains(&(combat::field::POWER1, 11620)) && f.contains(&(combat::field::LEVEL, 80)));
        let dk = Kit::plain(6, classes::stats(classes::by_word("dk").unwrap(), 80));
        let f = class_fields(&dk, false);
        assert_eq!(f[0].1 >> 24, 6, "runic power");
        assert!(f.contains(&(combat::field::POWER1 + 6, 1000)));
        assert!(matches!(class_enum(11), Class::Druid) && matches!(class_enum(0), Class::Warrior));
    }
}
