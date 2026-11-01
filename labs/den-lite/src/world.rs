//! The world server (port 8085): the auth session, one ready-made character, the login sequence a 3.3.5a client
//! needs to enter the world, the Den's room and props as gameobjects, movement, and the chat commands.
//!
//! Frames are read and written here (the header encryption from `wow_srp`); each message body is made or read by
//! `wow_world_messages`, except the two in `raw.rs`. One player at a time is enough for the prototype: the Den is
//! shared state, so a second connection sees the same props, but nobody sees anybody else (see the README).

use crate::den::{self, Change, WorldObject};
use crate::raw;
use crate::{Gender as HeroGender, Shared};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::{ProofSeed, ServerDecrypterHalf, ServerEncrypterHalf};
use wow_world_messages::wrath::opcodes::ClientOpcodeMessage;
use wow_world_messages::wrath::*;
use wow_world_messages::{DateTime, Guid};

pub const PLAYER_GUID: u64 = 1;
/// The GM island's area (AreaTable 876), map 1.
const AREA: Area = Area::GmIsland;
/// How many gameobject creates go in one SMSG_UPDATE_OBJECT.
const PER_UPDATE: usize = 40;

/// The outfit: (equipment slot, item entry, item display id, inventory type). From AzerothCore's world database,
/// `item_template` (entry, displayid, InventoryType): 4334 Formal White Shirt (7903, shirt), 39 Recruit's Pants
/// (9892, legs), 40 Recruit's Boots (10141, feet). The client draws them from its own Item.dbc by entry.
pub const OUTFIT: &[(usize, u32, u32, InventoryType)] =
    &[(3, 4334, 7903, InventoryType::Body), (6, 39, 9892, InventoryType::Legs), (7, 40, 10141, InventoryType::Feet)];

pub fn serve(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(300)));
            if let Err(e) = session(stream, &shared) {
                if e.kind() != io::ErrorKind::UnexpectedEof {
                    shared.log(&format!("world: {e}"));
                }
            }
        });
    }
}

fn bad(what: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.into())
}

/// The writing half of a connection: the stream and the header encrypter, shared with the session's _watcher thread
/// (the launcher's open.json), so either may send.
#[derive(Clone)]
pub struct Writer(Arc<Mutex<(TcpStream, Option<ServerEncrypterHalf>)>>);

impl Writer {
    fn send<M: ServerMessage>(&self, m: &M) -> io::Result<()> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut v = Vec::new();
        match g.1.as_mut() {
            Some(e) => m.write_encrypted_server(&mut v, e)?,
            None => m.write_unencrypted_server(&mut v)?,
        }
        g.0.write_all(&v)
    }

    fn send_raw(&self, opcode: u16, body: &[u8]) -> io::Result<()> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let e = g.1.as_mut().ok_or_else(|| bad("not signed in"))?;
        let mut v = Vec::with_capacity(body.len() + 5);
        e.write_encrypted_server_header(&mut v, body.len() as u32 + 2, opcode)?;
        v.extend_from_slice(body);
        g.0.write_all(&v)
    }
}

struct Conn {
    s: TcpStream,
    dec: Option<ServerDecrypterHalf>,
    w: Writer,
}

impl Conn {
    /// One client frame: (opcode, body).
    fn read(&mut self) -> io::Result<(u32, Vec<u8>)> {
        let (size, opcode) = match self.dec.as_mut() {
            Some(d) => {
                let h = d.read_and_decrypt_client_header(&mut self.s)?;
                (h.size, h.opcode)
            }
            None => {
                let mut h = [0u8; 6];
                self.s.read_exact(&mut h)?;
                (u16::from_be_bytes([h[0], h[1]]), u32::from_le_bytes([h[2], h[3], h[4], h[5]]))
            }
        };
        if size < 4 {
            return Err(bad("short frame"));
        }
        let mut body = vec![0u8; size as usize - 4];
        self.s.read_exact(&mut body)?;
        Ok((opcode, body))
    }

    fn send<M: ServerMessage>(&self, m: &M) -> io::Result<()> {
        self.w.send(m)
    }

    fn send_raw(&self, opcode: u16, body: &[u8]) -> io::Result<()> {
        self.w.send_raw(opcode, body)
    }
}

/// Stops the session's _watcher when the session ends, however it ends.
struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// The client's frame again as one buffer the message crate reads (header in the clear).
fn parse_client(opcode: u32, body: &[u8]) -> Option<ClientOpcodeMessage> {
    let mut v = Vec::with_capacity(body.len() + 6);
    v.extend_from_slice(&((body.len() + 4) as u16).to_be_bytes());
    v.extend_from_slice(&opcode.to_le_bytes());
    v.extend_from_slice(body);
    std::panic::catch_unwind(|| ClientOpcodeMessage::read_unencrypted(&v[..]).ok()).ok().flatten()
}

/// Where the player stands (from their own movement packets).
#[derive(Debug, Clone, Copy)]
struct Pos {
    x: f32,
    y: f32,
    z: f32,
    o: f32,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[allow(unused_assignments)]
fn session(s: TcpStream, shared: &Arc<Shared>) -> io::Result<()> {
    let w = Writer(Arc::new(Mutex::new((s.try_clone()?, None))));
    let mut c = Conn { s, dec: None, w };
    let seed = ProofSeed::new();
    c.send(&SMSG_AUTH_CHALLENGE { unknown1: 1, server_seed: seed.seed(), seed: crate::login::rand_bytes() })?;
    let (op, body) = c.read()?;
    let Some(ClientOpcodeMessage::CMSG_AUTH_SESSION(auth)) = parse_client(op, &body) else {
        return Err(bad("expected CMSG_AUTH_SESSION"));
    };
    let user = NormalizedString::new(&auth.username).map_err(|_| bad("bad account name"))?;
    let key = shared.sessions.lock().unwrap().get(&user.to_string()).copied();
    let Some(key) = key else {
        c.send(&SMSG_AUTH_RESPONSE::AuthUnknownAccount)?;
        return Ok(());
    };
    let crypto = seed
        .into_server_header_crypto(&user, key, auth.client_proof, auth.client_seed)
        .map_err(|_| bad("the session proof doesn't match"))?;
    let (enc, dec) = crypto.split();
    c.dec = Some(dec);
    c.w.0.lock().unwrap().1 = Some(enc);
    c.send(&SMSG_AUTH_RESPONSE::AuthOk {
        billing_flags: BillingPlanFlags::empty(),
        billing_rested: 0,
        billing_time: 0,
        expansion: Expansion::WrathOfTheLichKing,
    })?;
    let addons = addon_count(&auth.addon_info);
    c.send(&SMSG_ADDON_INFO {
        addons: (0..addons).map(|_| Addon { addon_type: 2, uses_crc: 1, uses_diffent_public_key: false, unknown1: 0, unknown2: 0 }).collect(),
    })?;
    c.send(&SMSG_CLIENTCACHE_VERSION { version: 0 })?;
    c.send(&SMSG_TUTORIAL_FLAGS { tutorial_data: [u32::MAX; 8] })?;
    shared.log(&format!("world: {user} connected"));

    let mut pos: Option<Pos> = None;
    // the open.json _watcher runs while this is Some (dropping it stops the thread)
    let mut _watcher: Option<StopOnDrop> = None;
    loop {
        let (op, body) = c.read()?;
        let Some(m) = parse_client(op, &body) else { continue };
        match m {
            ClientOpcodeMessage::CMSG_READY_FOR_ACCOUNT_DATA_TIMES => c.send(&account_times(CacheMask::GLOBAL_CACHE))?,
            ClientOpcodeMessage::CMSG_CHAR_ENUM => c.send(&SMSG_CHAR_ENUM { characters: vec![character(shared)] })?,
            ClientOpcodeMessage::CMSG_REALM_SPLIT(r) => c.send(&SMSG_REALM_SPLIT {
                realm_id: r.realm_id,
                state: RealmSplitState::Normal,
                split_date: "01/01/01".into(),
            })?,
            ClientOpcodeMessage::CMSG_PLAYER_LOGIN(p) => {
                if p.guid.guid() != PLAYER_GUID {
                    c.send(&SMSG_CHARACTER_LOGIN_FAILED { result: WorldResult::CharLoginNoCharacter })?;
                    continue;
                }
                let spot = shared.den.lock().unwrap().spot;
                let start = Pos { x: spot.x, y: spot.y, z: spot.z + 1.0, o: std::f32::consts::FRAC_PI_2 };
                enter_world(&c.w, shared, start)?;
                pos = Some(start);
                _watcher = Some(watch_open(shared, c.w.clone()));
            }
            ClientOpcodeMessage::CMSG_PING(p) => c.send(&SMSG_PONG { sequence_id: p.sequence_id })?,
            ClientOpcodeMessage::CMSG_NAME_QUERY(q) if q.guid.guid() == PLAYER_GUID => c.send(&SMSG_NAME_QUERY_RESPONSE {
                guid: q.guid,
                character_name: shared.cfg.hero.name.clone(),
                realm_name: String::new(),
                race: Race::Human,
                gender: hero_gender(shared),
                class: Class::Warrior,
                has_declined_names: SMSG_NAME_QUERY_RESPONSE_DeclinedNames::No,
            })?,
            ClientOpcodeMessage::CMSG_GAMEOBJECT_QUERY(q) => {
                let t = den::template(q.entry_id);
                let body = raw::gameobject_query_response(q.entry_id, t.map(|t| (t.kind, t.display, t.name, t.size)));
                c.send_raw(raw::SMSG_GAMEOBJECT_QUERY_RESPONSE, &body)?;
            }
            ClientOpcodeMessage::CMSG_UPDATE_ACCOUNT_DATA(u) => {
                c.send(&SMSG_UPDATE_ACCOUNT_DATA_COMPLETE { data_type: u.data_type, unknown1: 0 })?
            }
            ClientOpcodeMessage::CMSG_LOGOUT_REQUEST => {
                c.send(&SMSG_LOGOUT_RESPONSE { result: LogoutResult::Success, speed: LogoutSpeed::Instant })?;
                c.send(&SMSG_LOGOUT_COMPLETE {})?;
                pos = None;
                _watcher = None;
            }
            ClientOpcodeMessage::CMSG_MESSAGECHAT(m) => {
                if let Some(p) = pos {
                    chat(&c.w, shared, p, &m.message)?;
                }
            }
            other => {
                if let (Some(p), Some(n)) = (pos.as_mut(), moved(&other)) {
                    *p = n;
                }
            }
        }
    }
}

/// A movement packet's new position (the client moves itself; the server keeps where it is).
fn moved(m: &ClientOpcodeMessage) -> Option<Pos> {
    macro_rules! info {
        ($($v:ident),*) => {
            match m {
                $(ClientOpcodeMessage::$v(x) => Some(&x.info),)*
                _ => None,
            }
        };
    }
    let i = info!(
        MSG_MOVE_START_FORWARD, MSG_MOVE_START_BACKWARD, MSG_MOVE_STOP, MSG_MOVE_START_STRAFE_LEFT, MSG_MOVE_START_STRAFE_RIGHT,
        MSG_MOVE_STOP_STRAFE, MSG_MOVE_JUMP, MSG_MOVE_START_TURN_LEFT, MSG_MOVE_START_TURN_RIGHT, MSG_MOVE_STOP_TURN,
        MSG_MOVE_START_PITCH_UP, MSG_MOVE_START_PITCH_DOWN, MSG_MOVE_STOP_PITCH, MSG_MOVE_SET_RUN_MODE, MSG_MOVE_SET_WALK_MODE,
        MSG_MOVE_FALL_LAND, MSG_MOVE_START_SWIM, MSG_MOVE_STOP_SWIM, MSG_MOVE_SET_FACING, MSG_MOVE_SET_PITCH, MSG_MOVE_HEARTBEAT,
        MSG_MOVE_START_ASCEND, MSG_MOVE_STOP_ASCEND, MSG_MOVE_START_DESCEND
    )?;
    let v = &i.position;
    [v.x, v.y, v.z, i.orientation].iter().all(|f| f.is_finite()).then_some(Pos { x: v.x, y: v.y, z: v.z, o: i.orientation })
}

/// How many addons the client listed in CMSG_AUTH_SESSION (u32 size, then zlib: u32 count, the addons...).
pub fn addon_count(blob: &[u8]) -> usize {
    use flate2::read::ZlibDecoder;
    if blob.len() < 4 {
        return 0;
    }
    let mut out = Vec::new();
    if ZlibDecoder::new(&blob[4..]).take(64 * 1024).read_to_end(&mut out).is_err() || out.len() < 4 {
        return 0;
    }
    (u32::from_le_bytes([out[0], out[1], out[2], out[3]]) as usize).min(512)
}

fn account_times(mask: u32) -> SMSG_ACCOUNT_DATA_TIMES {
    let mut m = CacheMask::default();
    for (i, d) in m.data_mut().iter_mut().enumerate() {
        if mask & (1 << i) != 0 {
            *d = Some(0);
        }
    }
    SMSG_ACCOUNT_DATA_TIMES { unix_time: now() as u32, unknown1: 1, mask: m }
}

fn hero_gender(shared: &Shared) -> Gender {
    match shared.cfg.hero.gender {
        HeroGender::Male => Gender::Male,
        HeroGender::Female => Gender::Female,
    }
}

fn display_id(shared: &Shared) -> i32 {
    // CreatureDisplayInfo: the human male and female player models.
    match shared.cfg.hero.gender {
        HeroGender::Male => 49,
        HeroGender::Female => 50,
    }
}

pub fn character(shared: &Shared) -> Character {
    let spot = shared.den.lock().unwrap().spot;
    let empty = CharacterGear { equipment_display_id: 0, inventory_type: InventoryType::NonEquip, enchantment: 0 };
    let mut equipment = [empty; 23];
    for &(slot, _, display, inv) in OUTFIT {
        equipment[slot] = CharacterGear { equipment_display_id: display, inventory_type: inv, enchantment: 0 };
    }
    Character {
        guid: Guid::new(PLAYER_GUID),
        name: shared.cfg.hero.name.clone(),
        race: Race::Human,
        class: Class::Warrior,
        gender: hero_gender(shared),
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
        level: Level::new(1),
        area: AREA,
        map: Map::Kalimdor,
        position: Vector3d { x: spot.x, y: spot.y, z: spot.z + 1.0 },
        guild_id: 0,
        flags: 0,
        recustomization_flags: 0,
        first_login: false,
        pet_display_id: 0,
        pet_level: Level::new(0),
        pet_family: CreatureFamily::None,
        equipment,
    }
}

/// The player's own create: the fields a client needs to draw and control its character.
fn player_create(shared: &Shared, p: Pos) -> SMSG_UPDATE_OBJECT {
    let mut b = UpdatePlayerBuilder::new()
        .set_object_guid(Guid::new(PLAYER_GUID))
        .set_object_scale_x(1.0)
        .set_unit_bytes_0(Race::Human, Class::Warrior, hero_gender(shared), Power::Rage)
        .set_unit_health(100)
        .set_unit_maxhealth(100)
        .set_unit_maxpower2(1000)
        .set_unit_level(1)
        .set_unit_factiontemplate(1)
        .set_unit_displayid(display_id(shared))
        .set_unit_nativedisplayid(display_id(shared))
        .set_unit_boundingradius(0.389)
        .set_unit_combatreach(1.5)
        .set_unit_mod_cast_speed(1.0)
        .set_unit_hoverheight(1.0)
        // UNIT_FLAG_PLAYER_CONTROLLED, UNIT_FLAG2_REGENERATE_POWER (Player::Create)
        .set_unit_flags(0x0000_0008)
        .set_unit_flags_2(0x0000_0800)
        .set_player_field_bytes(0, 0, 0, 0)
        .set_player_bytes_2(0, 0, 0, 2)
        .set_player_bytes_3(if shared.cfg.hero.gender == HeroGender::Female { 1 } else { 0 }, 0, 0, 0)
        .set_player_next_level_xp(400)
        .set_player_watched_faction_index(-1);
    for &(slot, entry, _, _) in OUTFIT {
        let idx = VisibleItemIndex::try_from(slot as u8).expect("an equipment slot");
        b = b.set_player_visible_item(VisibleItem::new(entry, [0, 0]), idx);
    }
    let living = MovementBlock_UpdateFlag_Living::Living {
        backwards_flight_speed: 4.5,
        backwards_running_speed: 4.5,
        backwards_swimming_speed: 2.5,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 7.0,
        orientation: p.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: p.x, y: p.y, z: p.z },
        running_speed: 7.0,
        swimming_speed: 4.722222,
        timestamp: 0,
        turn_rate: std::f32::consts::PI,
        walking_speed: 2.5,
    };
    SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject2 {
            guid3: Guid::new(PLAYER_GUID),
            mask2: UpdateMask::Player(b.finalize()),
            movement2: MovementBlock { update_flag: MovementBlock_UpdateFlag::new_self().set_living(living) },
            object_type: ObjectType::Player,
        }],
    }
}

fn login_time() -> DateTime {
    let (y, mo, d, h, mi, _, wd) = den::civil(now());
    // packed as WorldPacket::AppendPackedTime: minutes, hours, weekday, day - 1, month - 1, years after 2000
    let packed = (mi & 0x3F) | (h & 0x1F) << 6 | (wd & 0x7) << 11 | ((d - 1) & 0x3F) << 14 | ((mo - 1) & 0xF) << 20 | (((y - 2000).clamp(0, 31) as u32) & 0x1F) << 24;
    DateTime::try_from(packed).unwrap_or_else(|_| DateTime::try_from(0).expect("a zero date"))
}

/// What the client needs from CMSG_PLAYER_LOGIN to the world on screen, in AzerothCore's order
/// (WorldSession::HandlePlayerLogin, Player::SendInitialPacketsBeforeAddToMap / AfterAddToMap).
fn enter_world(c: &Writer, shared: &Shared, p: Pos) -> io::Result<()> {
    let spot = shared.den.lock().unwrap().spot;
    c.send(&SMSG_LOGIN_VERIFY_WORLD { map: Map::Kalimdor, position: Vector3d { x: p.x, y: p.y, z: p.z }, orientation: p.o })?;
    c.send(&account_times(CacheMask::PER_CHARACTER_CACHE))?;
    c.send(&SMSG_FEATURE_SYSTEM_STATUS { complaint_status: ComplaintStatus::EnabledWithAutoIgnore, voice_chat_enabled: false })?;
    c.send(&SMSG_MOTD { motds: vec!["Welcome to your Den.".into()] })?;
    c.send(&SMSG_BINDPOINTUPDATE { position: Vector3d { x: spot.x, y: spot.y, z: spot.z + 1.0 }, map: Map::Kalimdor, area: AREA })?;
    c.send(&SMSG_INITIAL_SPELLS { unknown1: 0, initial_spells: vec![], cooldowns: vec![] })?;
    c.send(&SMSG_ACTION_BUTTONS::Initial { data: [ActionButton { action: 0, action_type: 0, misc: 0 }; 144] })?;
    c.send(&SMSG_INITIALIZE_FACTIONS { factions: vec![FactionInitializer { flag: FactionFlag::empty(), standing: 0 }; 128] })?;
    c.send(&SMSG_LOGIN_SETTIMESPEED { datetime: login_time(), timescale: 1.0 / 60.0, unknown1: 0 })?;
    c.send(&SMSG_TIME_SYNC_REQ { time_sync: 0 })?;
    c.send(&player_create(shared, p))?;
    c.send(&SMSG_INIT_WORLD_STATES { map: Map::Kalimdor, area: AREA, sub_area: AREA, states: vec![] })?;
    let objs = shared.den.lock().unwrap().everything();
    send_creates(c, &objs)?;
    say(c, &format!("Welcome to your Den: {} props. Type .den for the commands.", shared.den.lock().unwrap().props.len()))
}

fn send_creates(c: &Writer, objs: &[WorldObject]) -> io::Result<()> {
    for body in raw::gameobject_creates(objs, PER_UPDATE) {
        c.send_raw(raw::SMSG_UPDATE_OBJECT, &body)?;
    }
    Ok(())
}

fn apply(c: &Writer, ch: &Change) -> io::Result<()> {
    for &g in &ch.destroyed {
        c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false })?;
    }
    send_creates(c, &ch.created)?;
    if !ch.message.is_empty() {
        say(c, &ch.message)?;
    }
    Ok(())
}

/// A system line in the chat frame.
fn say(c: &Writer, text: &str) -> io::Result<()> {
    c.send(&SMSG_MESSAGECHAT {
        chat_type: SMSG_MESSAGECHAT_ChatType::System { target6: Guid::new(0) },
        language: Language::Universal,
        sender: Guid::new(0),
        flags: 0,
        message: text.into(),
        tag: PlayerChatTag::None,
    })
}

const HELP: &[&str] = &[
    ".den stamps: the stamps; .den stamp <name> now [degrees]: put one down where you stand",
    ".build undo: take the last change back",
    ".den save <name> / .den load <name> [replace|add] / .den files: your Den in a file",
    ".den room white|stone|wood: the room's look",
];

fn chat(c: &Writer, shared: &Shared, p: Pos, text: &str) -> io::Result<()> {
    let words: Vec<String> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    match (w(0), w(1)) {
        (".den", "") | (".den", "help") => {
            for l in HELP {
                say(c, l)?;
            }
            Ok(())
        }
        (".den", "stamps") => {
            let names = den::stamp_names(&shared.cfg.stamps_dir);
            say(c, &format!("Stamps: {}.", if names.is_empty() { "none here".into() } else { names.join(", ") }))
        }
        (".den", "stamp") => {
            if w(2).is_empty() {
                return say(c, "Which stamp? .den stamps lists them.");
            }
            let deg = words.iter().skip(3).find_map(|x| x.parse::<f32>().ok()).unwrap_or(0.0);
            if !deg.is_finite() || deg.abs() > 360.0 {
                return say(c, "Turn by up to 360 degrees.");
            }
            match den::read_stamp(&shared.cfg.stamps_dir, w(2)) {
                Err(e) => say(c, &e),
                Ok(st) => {
                    let ch = shared.den.lock().unwrap().stamp(&st, p.x, p.y, p.z, p.o + deg.to_radians());
                    apply(c, &ch)
                }
            }
        }
        (".build", "undo") => {
            let ch = shared.den.lock().unwrap().undo();
            apply(c, &ch)
        }
        (".den", "save") => {
            let r = shared.den.lock().unwrap().save(&shared.cfg.den_dir, w(2), now());
            match r {
                Ok((path, n)) => say(c, &format!("Saved to a file ({n} props): {}", path.display())),
                Err(e) => say(c, &e),
            }
        }
        (".den", "files") => {
            let names = den::list_names(&shared.cfg.den_dir);
            say(c, &format!("Saved Dens: {}.", if names.is_empty() { "none yet".into() } else { names.join(", ") }))
        }
        (".den", "load") => {
            let Some(name) = den::clean_name(w(2)) else { return say(c, "Which Den? .den files lists them.") };
            let doc = match den::read_file(&shared.cfg.den_dir.join(format!("{name}.den.json"))) {
                Ok(d) => d,
                Err(e) => return say(c, &e),
            };
            let mut d = shared.den.lock().unwrap();
            let replace = match w(3) {
                "replace" => true,
                "add" => false,
                _ if d.props.is_empty() => true,
                _ => {
                    let n = d.props.len();
                    drop(d);
                    return say(c, &format!("Your Den has {n} props: .den load {name} replace (a copy is kept as 'backup') or .den load {name} add."));
                }
            };
            if replace && !d.props.is_empty() {
                let _ = d.save(&shared.cfg.den_dir, "backup", now());
            }
            let ch = d.open(&doc, replace);
            drop(d);
            apply(c, &ch)
        }
        (".den", "room") => match den::look(w(2)) {
            None => say(c, "Looks: white, stone, wood."),
            Some(l) => {
                let mut d = shared.den.lock().unwrap();
                d.look = l;
                let destroyed = d.build_room();
                let ch = Change { created: d.room.clone(), destroyed, skipped: 0, message: format!("The room is {} now.", l.name) };
                drop(d);
                apply(c, &ch)
            }
        },
        (".den", _) | (".build", _) => say(c, "Den Lite knows: .den stamps, .den stamp, .build undo, .den save, .den load, .den files, .den room."),
        _ => c.send(&SMSG_MESSAGECHAT {
            chat_type: SMSG_MESSAGECHAT_ChatType::Say { target6: Guid::new(PLAYER_GUID) },
            language: Language::Universal,
            sender: Guid::new(PLAYER_GUID),
            flags: 0,
            message: text.into(),
            tag: PlayerChatTag::None,
        }),
    }
}

/// While the player is in the world: every 2 seconds, the launcher's `open.json` in the Dens folder
/// (`{"file": "<name>", "id": "<letters>"}`, as `ov_den.lua` reads it). A new id opens that file, replacing the Den
/// (a copy of what was there is kept as "backup"); each id opens once, also across restarts (`.open-done`).
fn watch_open(shared: &Arc<Shared>, w: Writer) -> StopOnDrop {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let dir = shared.cfg.den_dir.clone();
    let shared = shared.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if let Some((file, id)) = den::open_request(&dir) {
                if shared.mark_opened(&dir, &id) && check_open(&shared, &w, &file).is_err() {
                    break;
                }
            }
            for _ in 0..20 {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    StopOnDrop(stop)
}

fn check_open(shared: &Shared, w: &Writer, file: &str) -> io::Result<()> {
    let doc = match den::read_file(&shared.cfg.den_dir.join(format!("{file}.den.json"))) {
        Ok(d) => d,
        Err(e) => return say(w, &format!("The launcher asked to open '{file}': {e}")),
    };
    say(w, &format!("Opening '{file}' from the launcher."))?;
    let mut d = shared.den.lock().unwrap();
    if !d.props.is_empty() {
        let _ = d.save(&shared.cfg.den_dir, "backup", now());
    }
    let ch = d.open(&doc, true);
    drop(d);
    apply(w, &ch)
}
