//! The world server (port 8085): the auth session, one ready-made character, the login sequence a 3.3.5a client
//! needs to enter the world, the Den's room and props as gameobjects, movement, and the chat commands.
//!
//! Frames are read and written here (the header encryption from `wow_srp`); each message body is made or read by
//! `wow_world_messages`, except the two in `raw.rs`.
//!
//! Friends (guests.rs, owner 1 Nov): the host and their guests are in the same Den and see each other: each player
//! in the world is created for the others (no self flag), their movement packets are passed on with the mover's
//! guid, what they say is heard by all, logging out destroys them for the others, and every change to the Den goes
//! to everyone. Only the host builds, unless they say `.den friends build on`. The companion and the launcher's
//! open.json follow the host only. The remote world listener (the relay tunnel's) takes guests only.

use crate::companion::{self, Asked, At};
use crate::den::{self, Change, WorldObject};
use crate::notice::{self, Item, Old, Route};
use crate::{buildtools, dojo, gizmo, movewatch, mymodels, ovmod, places, play, raw, testctl};
use crate::{Gender as HeroGender, Lock, Shared};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::{ProofSeed, ServerDecrypterHalf, ServerEncrypterHalf};
use wow_world_messages::wrath::opcodes::ClientOpcodeMessage;
use wow_world_messages::wrath::*;
use wow_world_messages::{DateTime, Guid};

/// The sandbox combat and the class picker (sandbox.rs: a child module, so these hooks stay small).
#[path = "sandbox.rs"]
mod sandbox;
/// Jaina's brain in the world (jaina_world.rs: a child module, so these hooks stay small; owner 4 Nov).
#[path = "jaina_world.rs"]
mod jaina_world;
use jaina_world::{brain_tick, build_or_offer, jaina_feed, jaina_panel, jaina_saw, jaina_word, step_send};

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

/// `remote`: the relay tunnel's listener (guests only).
pub fn serve(listener: TcpListener, shared: Arc<Shared>, remote: bool) {
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(300)));
            let remote = remote || !stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
            if let Err(e) = session(stream, &shared, remote) {
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

    /// The connection closed (hosting.rs: the host's Kick). The session's read fails and it leaves as on a logout.
    pub fn close(&self) {
        let g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = g.0.shutdown(std::net::Shutdown::Both);
    }

    /// This connection, as notice.rs keeps lines for it.
    fn addr(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
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
type Pos = At;

/// The character a session plays: the host's ready-made one, or a guest's.
#[derive(Debug, Clone)]
pub struct Me {
    pub guid: u64,
    pub name: String,
    pub gender: HeroGender,
    pub host: bool,
}

impl Me {
    fn host(shared: &Shared) -> Me {
        Me { guid: PLAYER_GUID, name: shared.cfg.hero.name.clone(), gender: shared.cfg.hero.gender, host: true }
    }
    fn guest(g: &crate::guests::Guest) -> Me {
        Me { guid: g.guid, name: g.name.clone(), gender: if g.guid % 2 == 0 { HeroGender::Male } else { HeroGender::Female }, host: false }
    }
}

/// A player in the world now (Shared::players).
#[derive(Clone)]
pub struct Peer {
    pub sess: u64,
    pub me: Me,
    pub w: Writer,
    pub pos: At,
}

/// Every other player in the world (not `sess`).
fn others(shared: &Shared, sess: u64) -> Vec<Peer> {
    shared.players.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|p| p.sess != sess).cloned().collect()
}

/// The map a connection's player is on (Shared::seen; None: not in the world yet).
fn map_of(shared: &Shared, w: &Writer) -> Option<u32> {
    shared.seen.lock().unwrap_or_else(|e| e.into_inner()).get(&w.addr()).map(|s| s.0)
}

/// A friend's connection (their game gets the player's own models as stand-ins: mymodels.rs).
fn is_guest(shared: &Shared, w: &Writer) -> bool {
    shared.seen.lock().unwrap_or_else(|e| e.into_inner()).get(&w.addr()).is_some_and(|s| s.1)
}

fn set_map(shared: &Shared, w: &Writer, map: u32, guest: bool) {
    shared.seen.lock().unwrap_or_else(|e| e.into_inner()).insert(w.addr(), (map, guest));
}

/// The Den's map now.
fn den_map(shared: &Shared) -> u32 {
    shared.den_map.load(Ordering::Relaxed)
}

/// Every other player on the same map as `w`'s (a friend in another city doesn't see your moves here).
fn others_here(shared: &Shared, sess: u64, w: &Writer) -> Vec<Peer> {
    let m = map_of(shared, w);
    others(shared, sess).into_iter().filter(|o| map_of(shared, &o.w) == m).collect()
}

/// When a session ends: its map and guest mark go (a new connection may get the same address).
struct ForgetOnDrop(Arc<Shared>, usize);
impl Drop for ForgetOnDrop {
    fn drop(&mut self) {
        self.0.seen.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.1);
    }
}

/// A map id as the message crate takes it (places.rs MAPS and any map the game has); the island's map otherwise.
fn wmap(id: u32) -> Map {
    Map::try_from(id).unwrap_or(Map::Kalimdor)
}

/// An AreaTable id as the message crate takes it; 0 or unknown: the island's.
fn warea(id: u32) -> Area {
    if id == 0 { AREA } else { Area::try_from(id).unwrap_or(AREA) }
}

/// Where the Den is: its spot, the floor at its middle plus the spawn lift, its map and area for the messages.
fn den_where(shared: &Shared) -> (den::Spot, f32, Map, Area) {
    let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
    let area = d.place.as_ref().map(|p| p.1).unwrap_or(0);
    (d.spot, d.spawn_z(0.0), wmap(d.spot.map), if d.spot.is_home() { AREA } else { warea(area) })
}

/// A client's movement packet as the others get it: the mover's packed guid, then its movement info as sent.
fn relay_body(guid: u64, body: &[u8]) -> Option<Vec<u8>> {
    let mask = *body.first()?;
    let skip = 1 + mask.count_ones() as usize;
    if body.len() < skip {
        return None;
    }
    let mut out = Vec::with_capacity(body.len() + 8);
    raw::packed_guid(&mut out, guid);
    out.extend_from_slice(&body[skip..]);
    Some(out)
}

/// A session's number (the companion follows the player of the newest one).
static SESSIONS: AtomicU64 = AtomicU64::new(1);

/// What a session knows that the commands use.
struct Sess {
    id: u64,
    /// The addon names the client listed in CMSG_AUTH_SESSION.
    addons: Vec<String>,
    me: Me,
}

/// When a session ends however it ends: it is no longer the player in the world, and the companion goes with it.
struct LeaveOnDrop(Arc<Shared>, u64);
impl Drop for LeaveOnDrop {
    fn drop(&mut self) {
        leave(&self.0, self.1);
    }
}

fn leave(shared: &Shared, id: u64) {
    shared.notices.lock().unwrap_or_else(|e| e.into_inner()).gone(id);
    // `.back`'s spots are this time in the world's (places.rs)
    places::forget(id);
    // gone for the others
    let gone = {
        let mut ps = shared.players.lock().unwrap_or_else(|e| e.into_inner());
        let g = ps.iter().find(|p| p.sess == id).map(|p| p.me.guid);
        ps.retain(|p| p.sess != id);
        g
    };
    if let Some(g) = gone {
        for o in others(shared, id) {
            let _ = o.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false });
        }
    }
    let mut live = shared.live.lock().unwrap_or_else(|e| e.into_inner());
    if live.as_ref().is_some_and(|l| l.0 == id) {
        *live = None;
        drop(live);
        // what the host's game did with its character this time (logout, or the game closed)
        if let Some(w) = shared.moves.lock().unwrap_or_else(|e| e.into_inner()).take().filter(|w| w.sess == id) {
            shared.event("moves", &shared.cfg.hero.name, &format!("{}; sent: {}", w.summary(), w.ops_line()));
        }
        shared.companion.lock().unwrap_or_else(|e| e.into_inner()).away();
        shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job = None;
        shared.autosave();
    } else if gone.is_some() {
        drop(live);
        jaina_feed(shared, crate::jaina::Event::GuestLeft);
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[allow(unused_assignments)]
fn session(s: TcpStream, shared: &Arc<Shared>, remote: bool) -> io::Result<()> {
    let w = Writer(Arc::new(Mutex::new((s.try_clone()?, None))));
    let mut c = Conn { s, dec: None, w };
    let seed = ProofSeed::new();
    c.send(&SMSG_AUTH_CHALLENGE { unknown1: 1, server_seed: seed.seed(), seed: crate::login::rand_bytes() })?;
    let (op, body) = c.read()?;
    let Some(ClientOpcodeMessage::CMSG_AUTH_SESSION(auth)) = parse_client(op, &body) else {
        return Err(bad("expected CMSG_AUTH_SESSION"));
    };
    let user = NormalizedString::new(&auth.username).map_err(|_| bad("bad account name"))?;
    let found = shared.sessions.lk().get(&user.to_string()).cloned();
    // the host's login never comes in from outside this PC (nor one made on the remote login listener in here)
    let found = found.filter(|(_, who, from_remote)| *who != crate::login::Who::Host || (!remote && !*from_remote));
    let Some((key, who, _)) = found else {
        c.send(&SMSG_AUTH_RESPONSE::AuthUnknownAccount)?;
        return Ok(());
    };
    let me = match &who {
        crate::login::Who::Host => Me::host(shared),
        crate::login::Who::Guest(g) => Me::guest(g),
    };
    let crypto = seed
        .into_server_header_crypto(&user, key, auth.client_proof, auth.client_seed)
        .map_err(|_| bad("the session proof doesn't match"))?;
    let (enc, dec) = crypto.split();
    c.dec = Some(dec);
    c.w.0.lk().1 = Some(enc);
    c.send(&SMSG_AUTH_RESPONSE::AuthOk {
        billing_flags: BillingPlanFlags::empty(),
        billing_rested: 0,
        billing_time: 0,
        expansion: Expansion::WrathOfTheLichKing,
    })?;
    let addons = addon_count(&auth.addon_info);
    let sess = Sess { id: SESSIONS.fetch_add(1, Ordering::Relaxed), addons: play::addon_names(&auth.addon_info), me };
    let _leave = LeaveOnDrop(shared.clone(), sess.id);
    let _forget = ForgetOnDrop(shared.clone(), c.w.addr());
    c.send(&SMSG_ADDON_INFO {
        // one entry per addon only when every key CRC is the standard one (AzerothCore would send its public key for
        // the others, which Den Lite doesn't have): the real game (2 Nov 01:00) sent 23 addons with CRC 0, and answered
        // without keys it never asked for its characters. With none listed it plays (the combat log box is closed by
        // the Leyline UI in the Den).
        addons: (0..if play::addon_odd_crcs(&auth.addon_info).is_empty() { addons } else { 0 })
            .map(|_| Addon { addon_type: 2, uses_crc: 1, uses_diffent_public_key: false, unknown1: 0, unknown2: 0 })
            .collect(),
    })?;
    c.send(&SMSG_CLIENTCACHE_VERSION { version: 0 })?;
    c.send(&SMSG_TUTORIAL_FLAGS { tutorial_data: [u32::MAX; 8] })?;
    shared.log(&format!("world: {user} connected"));
    shared.event("world_auth", &user.to_string(), "");
    let odd = play::addon_odd_crcs(&auth.addon_info);
    shared.event("addon_crcs", &user.to_string(), &format!("{} of {addons} not standard{}{}", odd.len(), if odd.is_empty() { "" } else { ": " }, odd.join(" ")));

    let mut pos: Option<Pos> = None;
    // the open.json _watcher runs while this is Some (dropping it stops the thread)
    let mut _watcher: Option<StopOnDrop> = None;
    // the Builder's selection (ovmod.rs), whether this session moved yet and was answered by the addon channel
    let mut sel = ovmod::Sel::default();
    // the editor's placement aids (gizmo.rs): the live preview, the gizmo, the exact click
    let mut aids = gizmo::Aids::new(gizmo::Camera::load(&shared.cfg.den_dir));
    let (mut moved_once, mut ready) = (false, false);
    // what the game does with its character (movewatch.rs: the events and `.den where`)
    let mut watch: Option<movewatch::Watch> = None;
    // after a teleport: a fall through the world is lifted back (places.rs Landing)
    let mut landing: Option<places::Landing> = None;
    // past the loading screen (notice.rs: the Den bar's hello is waited for from then)
    let mut loaded = false;
    loop {
        let (op, body) = c.read()?;
        if !loaded && pos.is_some() && (op == 0x026A || op == 0x0391 || raw_move(op, &body).is_some()) {
            loaded = shared.notices.lock().unwrap_or_else(|e| e.into_inner()).loaded(sess.id, now_ms() + shared.cfg.ui_wait_ms);
            if loaded {
                shared.event("loaded", &sess.me.name, &format!("{op:#x}"));
            }
        }
        // everything the game sends once in the world, written down once per opcode (3 Nov: "no movement packets at
        // all"; the log now says what it did send), and its answer to the time sync
        if pos.is_some() {
            if let Some(w) = watch.as_mut().filter(|w| w.sess == sess.id) {
                if let Some(name) = w.client_op(op) {
                    shared.event("client_op", &sess.me.name, &name);
                }
                if op == movewatch::CMSG_TIME_SYNC_RESP {
                    if let Some(d) = w.synced(&body) {
                        shared.event("time_sync", &sess.me.name, &d);
                    }
                }
                if sess.me.host {
                    *shared.moves.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
                }
            }
        }
        // the game says it moves its character now (the world is on screen): told that it has control of it, and
        // unrooted (movewatch.rs; 3 Nov: the character hung in the air and wouldn't walk)
        if op == movewatch::CMSG_SET_ACTIVE_MOVER && pos.is_some() {
            if let Some(w) = watch.as_mut().filter(|w| w.sess == sess.id) {
                if w.mover() {
                    let guid = body.get(..8).map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]));
                    shared.event("mover", &sess.me.name, if guid == Some(sess.me.guid) { "the game moves its character" } else { "the game named another guid" });
                    c.send_raw(movewatch::SMSG_CLIENT_CONTROL_UPDATE, &movewatch::control_body(sess.me.guid))?;
                    c.send_raw(movewatch::SMSG_FORCE_MOVE_UNROOT, &movewatch::unroot_body(sess.me.guid, 0))?;
                    if sess.me.host {
                        *shared.moves.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
                    }
                }
            }
        }
        // `.den go` (places.rs): the game's answer to a teleport. Near: it moved its character; far: the new map is
        // loaded, and everything is sent again (arrive_far)
        if (op == places::MSG_MOVE_TELEPORT_ACK as u32 || op == places::MSG_MOVE_WORLDPORT_ACK as u32) && pos.is_some() {
            let trip = shared.trips.lock().unwrap_or_else(|e| e.into_inner()).remove(&sess.id);
            if let Some(t) = trip {
                let near_ok = op == places::MSG_MOVE_TELEPORT_ACK as u32
                    && !t.far
                    && places::teleport_ack(&body).is_some_and(|(g, n)| g == sess.me.guid && n == t.counter);
                let far_ok = op == places::MSG_MOVE_WORLDPORT_ACK as u32 && t.far;
                if near_ok || far_ok {
                    let at = At { x: t.x, y: t.y, z: t.z, o: t.o };
                    pos = Some(at);
                    if let Some(l) = shared.live.lk().as_mut().filter(|l| l.0 == sess.id) {
                        l.2 = at;
                    }
                    if let Some(me) = shared.players.lk().iter_mut().find(|x| x.sess == sess.id) {
                        me.pos = at;
                    }
                    shared.event("arrived", &sess.me.name, &format!("{} map {} {:.1} {:.1} {:.1}: {}", if t.far { "far" } else { "near" }, t.map, t.x, t.y, t.z, t.title));
                    // Den Lite knows no ground there: watched (a lift keeps its count, so lifts end)
                    if t.title != places::LIFTED || landing.is_none() {
                        landing = Some(places::Landing::new(t.map, t.x, t.y, t.z));
                    }
                    if far_ok {
                        // a new time in a world: the game takes its character again (SET_ACTIVE_MOVER: control said
                        // once more), and its moves are watched afresh (movewatch.rs)
                        let floor = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(at.x, at.y);
                        let w = movewatch::Watch::new(sess.id, at, if t.map == den_map(shared) { floor } else { at.z });
                        if sess.me.host {
                            *shared.moves.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
                        }
                        watch = Some(w);
                        gizmo::clear(&mut aids);
                        arrive_far(&c.w, shared, &sess, &t)?;
                        // a new world forgets flying and the speed: they come back as they were (the free camera
                        // and flying work anywhere; docs/DEN-PLACES.md)
                        moves_again(&c.w, shared, &sess)?;
                    } else {
                        for o in others_here(shared, sess.id, &c.w) {
                            let _ = o.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(sess.me.guid), target_died: false });
                            let _ = o.w.send(&player_create(&sess.me, at, false));
                        }
                    }
                    if t.title != places::LIFTED {
                        say(&c.w, &format!("You are at {}. .back goes back, .den home to your Den; .den here makes your Den here.", t.title))?;
                    }
                    places_line(&c.w, shared, &sess)?;
                } else {
                    // not this trip's answer: it still waits
                    shared.trips.lock().unwrap_or_else(|e| e.into_inner()).insert(sess.id, t);
                }
            }
            continue;
        }
        // the sandbox combat (sandbox.rs, combat.rs): casts, the target, swings, the bar; a start of movement stops a cast
        if sandbox::client_op(&c.w, shared, &sess, pos, op, &body)? {
            continue;
        }
        // the game's answers to the Dojo's fly and speed changes (dojo.rs): counted for the log, and where the
        // character is (nothing waits for them)
        if dojo::ACKS.contains(&op) {
            let what = match op {
                dojo::CMSG_MOVE_SET_CAN_FLY_ACK => "fly",
                dojo::CMSG_FORCE_RUN_SPEED_CHANGE_ACK => "run speed",
                _ => "flight speed",
            };
            if sess.me.host {
                let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
                if op == dojo::CMSG_MOVE_SET_CAN_FLY_ACK { d.acks.0 += 1 } else { d.acks.1 += 1 }
            }
            shared.event("dojo_ack", &sess.me.name, &format!("{what} {}", dojo::ack_counter(&body).unwrap_or(0)));
            if let (Some(p), Some(n)) = (pos.as_mut(), dojo::ack_pos(&body)) {
                *p = n;
                if let Some(l) = shared.live.lk().as_mut().filter(|l| l.0 == sess.id) {
                    l.2 = n;
                }
                if let Some(me) = shared.players.lk().iter_mut().find(|x| x.sess == sess.id) {
                    me.pos = n;
                }
            }
            continue;
        }
        // the game's own settings (accdata.rs): unanswered, it had no key bindings and the character never moved
        if op == crate::accdata::CMSG_REQUEST && body.len() >= 4 {
            let t = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
            if t < crate::accdata::TYPES {
                let a = crate::accdata::answer(&shared.cfg.den_dir, &user.to_string(), &sess.me.name, sess.me.guid, t);
                shared.event("account_data", &sess.me.name, &format!("ask {t} {}", if a.len() > 20 { "kept" } else { "none kept" }));
                c.send_raw(crate::accdata::SMSG_UPDATE, &a)?;
            }
            continue;
        }
        if op == crate::accdata::CMSG_UPDATE {
            if let Some(t) = crate::accdata::store(&shared.cfg.den_dir, &user.to_string(), &sess.me.name, &body) {
                shared.event("account_data", &sess.me.name, &format!("kept {t}"));
                let mut done = Vec::with_capacity(8);
                done.extend_from_slice(&t.to_le_bytes());
                done.extend_from_slice(&0u32.to_le_bytes());
                c.send_raw(crate::accdata::SMSG_COMPLETE, &done)?;
            }
            continue;
        }
        // the editor's exact click (the ground-target cursor's cast) and a right-click on a prop or a gizmo flag
        // (gizmo.rs); a cast of any other spell goes on as before
        if pos.is_some() && (op == gizmo::CMSG_CAST_SPELL || op == gizmo::CMSG_GAMEOBJ_USE || op == gizmo::CMSG_GAMEOBJ_REPORT_USE) {
            if aid_packet(&c.w, shared, &sess, pos, &mut sel, &mut aids, op, &body)? {
                continue;
            }
        }
        if op == ovmod::CMSG_MESSAGECHAT {
            if let Some(m) = ovmod::parse_addon(&body) {
                addon(&c.w, shared, &sess, pos, &mut sel, &mut aids, &mut ready, &m)?;
                continue;
            }
        }
        let Some(m) = parse_client(op, &body) else {
            // a movement packet the message crate didn't read (the real game's, 1 Nov 23:43: not one move seen):
            // where the player went, read by hand (packed guid, flags, flags2, time, x, y, z, o)
            if let (Some(p), Some(n)) = (pos.as_mut(), raw_move(op, &body)) {
                *p = n;
                moved_once = true;
                track(shared, &sess, &mut watch, op, &body, n);
                watch_landing(&c.w, shared, &sess, &mut landing, &body, n)?;
                aid_follow(&c.w, shared, &sess, &sel, &mut aids, n)?;
                if let Some(l) = shared.live.lk().as_mut().filter(|l| l.0 == sess.id) {
                    l.2 = n;
                }
                if let Some(me) = shared.players.lk().iter_mut().find(|x| x.sess == sess.id) {
                    me.pos = n;
                }
                // the others see the move (not the game's own "I fly now", which is only for the server)
                if let (Ok(code), Some(b), true) = (u16::try_from(op), relay_body(sess.me.guid, &body), op != dojo::CMSG_MOVE_SET_FLY) {
                    for o in others_here(shared, sess.id, &c.w) {
                        let _ = o.w.send_raw(code, &b);
                    }
                }
            } else if raw_move(op, &body).is_some() && !moved_once {
                moved_once = true;
                shared.event("first_move", &sess.me.name, "raw");
            }
            continue;
        };
        match m {
            ClientOpcodeMessage::CMSG_READY_FOR_ACCOUNT_DATA_TIMES => c.send(&account_times(shared, &user.to_string(), &sess.me.name, CacheMask::GLOBAL_CACHE))?,
            ClientOpcodeMessage::CMSG_CHAR_ENUM => {
                c.send(&SMSG_CHAR_ENUM { characters: vec![character(shared, &sess.me)] })?;
                shared.event("char_enum", &sess.me.name, "1");
            }
            // Den Lite keeps its one ready-made character: a made one is "made" and the list shows that one
            ClientOpcodeMessage::CMSG_CHAR_CREATE(_) => {
                c.send(&SMSG_CHAR_CREATE { result: WorldResult::CharCreateSuccess })?;
                shared.event("char_create", &sess.me.name, "");
            }
            ClientOpcodeMessage::CMSG_REALM_SPLIT(r) => c.send(&SMSG_REALM_SPLIT {
                realm_id: r.realm_id,
                state: RealmSplitState::Normal,
                split_date: "01/01/01".into(),
            })?,
            ClientOpcodeMessage::CMSG_PLAYER_LOGIN(p) => {
                shared.event("player_login", &sess.me.name, "");
                if p.guid.guid() != sess.me.guid {
                    c.send(&SMSG_CHARACTER_LOGIN_FAILED { result: WorldResult::CharLoginNoCharacter })?;
                    continue;
                }
                // a guest a step to the side of the host's spot; on the ground (movewatch::SPAWN_LIFT), not a yard up
                let side = if sess.me.host { 0.0 } else { 2.0 + (sess.me.guid % 4) as f32 };
                let (start, ground, file, near) = {
                    let d = shared.den.lk();
                    let start = At { x: d.spot.x + side, y: d.spot.y, z: d.spawn_z(side), o: std::f32::consts::FRAC_PI_2 };
                    (start, d.floor_at(start.x, start.y), d.ground_here().is_some(), d.nearest_object(start.x, start.y))
                };
                shared.event("spawn", &sess.me.name, &movewatch::spawn_note(start, ground, file, near));
                // the player comes in at the Den, wherever it is (places.rs: `.den here`)
                set_map(shared, &c.w, den_map(shared), !sess.me.host);
                let w = movewatch::Watch::new(sess.id, start, ground);
                if sess.me.host {
                    *shared.moves.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
                }
                watch = Some(w);
                if sess.me.host {
                    sandbox::prepare(shared);
                }
                enter_world(&c.w, shared, &user.to_string(), &sess.me, start)?;
                arrive(&c.w, shared, &sess, start)?;
                if !sess.me.host {
                    jaina_feed(shared, crate::jaina::Event::GuestArrived);
                }
                // the Dojo: the game starts on foot at its normal speed; the training dummies up are shown
                if sess.me.host {
                    shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).entered();
                }
                let dummies = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).dummies.clone();
                for d in &dummies {
                    c.send(&dummy_create(shared, d))?;
                }
                pos = Some(start);
                loaded = false;
                shared.event("in_world", &sess.me.name, &format!("{:.1} {:.1} {:.1}", start.x, start.y, start.z));
                // the others on this map: they see this player, this player sees them
                for o in others_here(shared, sess.id, &c.w) {
                    c.send(&player_create(&o.me, o.pos, false))?;
                    shared.event("peer_seen", &sess.me.name, &o.me.name);
                    if o.w.send(&player_create(&sess.me, start, false)).is_ok() {
                        shared.event("peer_seen", &o.me.name, &sess.me.name);
                    }
                    if !sess.me.host {
                        let _ = notify(&o.w, shared, o.sess, o.me.guid, Item::line("ok", format!("{} is in your Den.", sess.me.name)));
                    }
                }
                shared.players.lk().push(Peer { sess: sess.id, me: sess.me.clone(), w: c.w.clone(), pos: start });
                if sess.me.host {
                    *shared.live.lk() = Some((sess.id, c.w.clone(), start));
                    _watcher = Some(watch_open(shared, c.w.clone()));
                }
            }
            ClientOpcodeMessage::CMSG_PING(p) => c.send(&SMSG_PONG { sequence_id: p.sequence_id })?,
            ClientOpcodeMessage::CMSG_NAME_QUERY(q) => {
                let g = q.guid.guid();
                let who = if g == sess.me.guid { Some(sess.me.clone()) } else if g == PLAYER_GUID { Some(Me::host(shared)) } else {
                    shared.players.lk().iter().find(|p| p.me.guid == g).map(|p| p.me.clone())
                };
                if let Some(m) = who {
                    let class = if m.host { sandbox::class_of(shared).0 } else { Class::Warrior };
                    c.send(&SMSG_NAME_QUERY_RESPONSE {
                        guid: q.guid,
                        character_name: m.name,
                        realm_name: String::new(),
                        race: Race::Human,
                        gender: gender_of(m.gender),
                        class,
                        has_declined_names: SMSG_NAME_QUERY_RESPONSE_DeclinedNames::No,
                    })?
                }
            }
            ClientOpcodeMessage::CMSG_GAMEOBJECT_QUERY(q) => {
                // a build twin (gizmo.rs `twin`: build mode shows props under it) is answered as a goober
                if gizmo::untwin(q.entry_id).is_some() {
                    shared.twin_queries.fetch_add(1, Ordering::Relaxed);
                }
                let body = raw::gameobject_query_response(q.entry_id, gizmo::query_any(q.entry_id));
                c.send_raw(raw::SMSG_GAMEOBJECT_QUERY_RESPONSE, &body)?;
            }
            ClientOpcodeMessage::CMSG_UPDATE_ACCOUNT_DATA(u) => {
                c.send(&SMSG_UPDATE_ACCOUNT_DATA_COMPLETE { data_type: u.data_type, unknown1: 0 })?
            }
            ClientOpcodeMessage::CMSG_LOGOUT_REQUEST => {
                shared.event("logout", &sess.me.name, "");
                if let Some(w) = watch.take() {
                    if !sess.me.host {
                        shared.event("moves", &sess.me.name, &format!("{}; sent: {}", w.summary(), w.ops_line()));
                    }
                }
                // out of the world first (the companion goes with the player), then the client is told
                gizmo::clear(&mut aids);
                leave(shared, sess.id);
                pos = None;
                _watcher = None;
                c.send(&SMSG_LOGOUT_RESPONSE { result: LogoutResult::Success, speed: LogoutSpeed::Instant })?;
                c.send(&SMSG_LOGOUT_COMPLETE {})?;
            }
            ClientOpcodeMessage::CMSG_CREATURE_QUERY(q) => c.send(&creature_answer(q.creature))?,
            ClientOpcodeMessage::CMSG_MESSAGECHAT(m) => {
                if let Some(p) = pos {
                    if sess.me.host {
                        chat(&c.w, shared, &sess, p, &m.message)?;
                    } else {
                        guest_chat(&c.w, shared, &sess, p, &m.message)?;
                    }
                }
            }
            other => {
                if let (Some(p), Some(n)) = (pos.as_mut(), moved(&other)) {
                    let was = *p;
                    *p = n;
                    moved_once = true;
                    track(shared, &sess, &mut watch, op, &body, n);
                    guard_rail(&c.w, shared, &sess, was, n, op, &body)?;
                    watch_landing(&c.w, shared, &sess, &mut landing, &body, n)?;
                    aid_follow(&c.w, shared, &sess, &sel, &mut aids, n)?;
                    if let Some(l) = shared.live.lk().as_mut().filter(|l| l.0 == sess.id) {
                        l.2 = n;
                    }
                    if let Some(me) = shared.players.lk().iter_mut().find(|x| x.sess == sess.id) {
                        me.pos = n;
                    }
                    // the others see the move
                    if let (Ok(code), Some(b)) = (u16::try_from(op), relay_body(sess.me.guid, &body)) {
                        for o in others_here(shared, sess.id, &c.w) {
                            let _ = o.w.send_raw(code, &b);
                        }
                    }
                }
            }
        }
    }
}

/// An addon message (ovmod.rs): the in-game Den panel and Builder against this Den.
#[allow(clippy::too_many_arguments)]
fn addon(c: &Writer, shared: &Shared, sess: &Sess, pos: Option<Pos>, sel: &mut ovmod::Sel, aids: &mut gizmo::Aids, ready: &mut bool, m: &ovmod::AddonMsg) -> io::Result<()> {
    if m.prefix != ovmod::PREFIX {
        return Ok(());
    }
    let here = pos.map(|p| ovmod::Here { x: p.x, y: p.y, z: p.z, o: p.o });
    let can = crate::hosting::may_build(shared, &sess.me);
    // the placement aids' verbs (gizmo.rs): hover comes about 10 a second, so it isn't written to the events
    if let Some(("wubuild", "cmd", data)) = ovmod::split(&m.text) {
        // the Builder's tools (buildtools.rs: paths, stamps with the mouse, arrays, align, Solid, layers, history)
        if buildtools::is_tool(data) {
            let Some(h) = here else { return Ok(()) };
            shared.event("addon", &sess.me.name, &format!("wubuild tool {}", data.split_whitespace().take(2).collect::<Vec<_>>().join(" ")));
            let (out, follow) = {
                let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
                let ctx = buildtools::Ctx { dens: &shared.cfg.den_dir, stamps: &shared.cfg.stamps_dir, who: &sess.me.name, unix: now() };
                let out = buildtools::cmd(&mut d, sel, aids, h, data, can, &ctx);
                // the gizmo goes where the selection now is (a box select, a path laid)
                (out, gizmo::sync(&d, sel, aids))
            };
            return quietly(c, shared, sess.id, sess.me.guid, || {
                aid_send(c, shared, sess, sel, aids, out)?;
                aid_send(c, shared, sess, sel, aids, follow)
            });
        }
        if gizmo::is_aid(data) {
            let Some(h) = here else { return Ok(()) };
            if !data.trim_start().to_ascii_lowercase().starts_with("hover") {
                shared.event("addon", &sess.me.name, &format!("wubuild aid {}", data.split_whitespace().take(2).collect::<Vec<_>>().join(" ")));
            }
            let out = {
                let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
                gizmo::cmd(&mut d, sel, aids, h, data, can, now_ms())
            };
            return quietly(c, shared, sess.id, sess.me.guid, || aid_send(c, shared, sess, sel, aids, out));
        }
    }
    let label: String = m.text.split('\t').take(2).collect::<Vec<_>>().join(" ");
    shared.event("addon", &sess.me.name, &label);
    let (outs, follow) = {
        let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let outs = ovmod::handle(&mut d, sel, here, m, sess.me.host, can);
        // the gizmo goes where the selection now is (a move, a pick, a delete)
        (outs, gizmo::sync(&d, sel, aids))
    };
    // what the bar or the Builder asked for answers in the bar (notice.rs), never in the chat
    quietly(c, shared, sess.id, sess.me.guid, || {
        addon_outs(c, shared, sess, pos, ready, outs)?;
        aid_send(c, shared, sess, sel, aids, follow)
    })
}

/// What the aids do (gizmo.rs AidOut): the Den's change for everyone, build mode's look for everyone, this player's
/// own objects (the copy, the gizmo: made, then the old ones taken away), the addon's lines, the state.
fn aid_send(c: &Writer, shared: &Shared, sess: &Sess, sel: &ovmod::Sel, aids: &gizmo::Aids, out: gizmo::AidOut) -> io::Result<()> {
    if let Some(ch) = &out.change {
        apply(c, shared, ch)?;
    }
    if let Some(build) = out.look {
        // the event every time the mode is asked for (the den-test reads it; the editor may already be in it)
        shared.event("build_mode", &sess.me.name, if build { "build" } else { "play" });
        if shared.build_look.swap(build, Ordering::Relaxed) != build {
            // every prop shown again in the new look, to everyone in the Den
            let objs: Vec<WorldObject> = shared.den.lock().unwrap_or_else(|e| e.into_inner()).props.iter().filter(|p| p.prop.hidden != Some(true)).map(|p| p.obj.clone()).collect();
            for p in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                for o in &objs {
                    let _ = p.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(o.guid), target_died: false });
                }
                let _ = send_creates(&p.w, shared, &objs);
            }
        }
    }
    if !out.create.is_empty() {
        send_creates(c, shared, &out.create)?;
    }
    for g in &out.destroy {
        c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(*g), target_died: false })?;
    }
    // the selection's glow (GAMEOBJECT_DYNAMIC) after its create; the far camera (gizmo.rs, farsight.rs)
    for (g, flags) in &out.glow {
        c.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::gameobject_dynamic(*g, *flags))?;
    }
    aid_far(c, sess, out.far)?;
    for (name, data) in &out.addon {
        send_addon(c, sess.me.guid, "wubuild", name, data)?;
    }
    if out.state {
        let role = if sess.me.host { "owner" } else if crate::hosting::may_build(shared, &sess.me) { "builder" } else { "visitor" };
        let st = ovmod::state(&shared.den.lock().unwrap_or_else(|e| e.into_inner()), sel, role);
        send_addon(c, sess.me.guid, "wubuild", "state", &st)?;
    }
    for s in &out.say {
        say(c, s)?;
    }
    if sess.me.host {
        *shared.aid_view.lock().unwrap_or_else(|e| e.into_inner()) = gizmo::view(aids, sel, shared.build_look.load(Ordering::Relaxed));
    }
    Ok(())
}

/// The far camera's object (farsight.rs): made there and the view on it; moved with a short flying spline; the view
/// given back and the object gone.
fn aid_far(c: &Writer, sess: &Sess, far: Option<gizmo::FarOut>) -> io::Result<()> {
    use crate::{farsight, flight};
    match far {
        None => Ok(()),
        Some(gizmo::FarOut::Show(at)) => {
            c.send(&farsight::create(farsight::at(at)))?;
            c.send_raw(flight::SMSG_SPLINE_MOVE_SET_FLYING, &flight::guid_body(farsight::GUID))?;
            c.send_raw(flight::SMSG_SPLINE_MOVE_GRAVITY_DISABLE, &flight::guid_body(farsight::GUID))?;
            c.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::farsight(sess.me.guid, farsight::GUID))
        }
        Some(gizmo::FarOut::Move(from, to)) => c.send_raw(flight::SMSG_MONSTER_MOVE, &farsight::move_body(from, to, (now_ms() & 0xFFFF_FFFF) as u32)),
        Some(gizmo::FarOut::Glide(from, to, ms)) => {
            c.send_raw(flight::SMSG_MONSTER_MOVE, &farsight::glide_body(from, to, (now_ms() & 0xFFFF_FFFF) as u32, ms))
        }
        Some(gizmo::FarOut::Off) => {
            c.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::farsight(sess.me.guid, 0))?;
            c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(farsight::GUID), target_died: false })
        }
    }
}

/// The player moved: an armed copy that isn't on the cursor follows them (gizmo.rs `follow_player`).
fn aid_follow(c: &Writer, shared: &Shared, sess: &Sess, sel: &ovmod::Sel, aids: &mut gizmo::Aids, n: Pos) -> io::Result<()> {
    if aids.armed.is_none() || !aids.follow {
        return Ok(());
    }
    let here = ovmod::Here { x: n.x, y: n.y, z: n.z, o: n.o };
    let out = gizmo::follow_player(&shared.den.lock().unwrap_or_else(|e| e.into_inner()), sel, aids, here, now_ms());
    if out == gizmo::AidOut::default() {
        return Ok(());
    }
    aid_send(c, shared, sess, sel, aids, out)
}

/// The exact click's cast (CMSG_CAST_SPELL of gizmo::PLACE_SPELL with a destination) and a right-click on a
/// gameobject (CMSG_GAMEOBJ_USE): true when it was the aids' (nothing else then sees it).
#[allow(clippy::too_many_arguments)]
fn aid_packet(c: &Writer, shared: &Shared, sess: &Sess, pos: Option<Pos>, sel: &mut ovmod::Sel, aids: &mut gizmo::Aids, op: u32, body: &[u8]) -> io::Result<bool> {
    let Some(p) = pos else { return Ok(false) };
    let here = ovmod::Here { x: p.x, y: p.y, z: p.z, o: p.o };
    let can = crate::hosting::may_build(shared, &sess.me);
    if op == gizmo::CMSG_CAST_SPELL {
        let Some(cast) = gizmo::parse_cast(body).filter(|c| c.spell == gizmo::PLACE_SPELL) else { return Ok(false) };
        // never cast: no message, no cooldown, no visual
        c.send_raw(gizmo::SMSG_CAST_FAILED, &gizmo::cast_failed_body(&cast))?;
        c.send_raw(gizmo::SMSG_CLEAR_COOLDOWN, &gizmo::clear_cooldown_body(gizmo::PLACE_SPELL, sess.me.guid))?;
        let Some(at) = cast.dest else { return Ok(true) };
        shared.event("aid_click", &sess.me.name, &format!("{:.2} {:.2} {:.2}", at[0], at[1], at[2]));
        // with the last hover: one sample for the build camera's calibration (den-check fits it)
        if let Some(s) = gizmo::sample(aids, here, at).filter(|_| shared.cfg.test_control) {
            if let (Ok(line), Ok(mut f)) = (serde_json::to_string(&s), std::fs::OpenOptions::new().create(true).append(true).open(shared.cfg.den_dir.join(gizmo::SAMPLES_FILE))) {
                let _ = writeln!(f, "{line}");
            }
        }
        let out = {
            let mut d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
            gizmo::dest(&mut d, sel, aids, at[0], at[1], now_ms(), can)
        };
        quietly(c, shared, sess.id, sess.me.guid, || aid_send(c, shared, sess, sel, aids, out))?;
        return Ok(true);
    }
    if op == gizmo::CMSG_GAMEOBJ_REPORT_USE {
        return Ok(true);
    }
    let Some(guid) = body.get(..8).map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])) else { return Ok(true) };
    let out = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        gizmo::use_object(&d, sel, aids, guid, shared.build_look.load(Ordering::Relaxed), now_ms())
    };
    if out != gizmo::AidOut::default() {
        shared.event("aid_use", &sess.me.name, &format!("{}", guid & 0xFF_FFFF));
    }
    quietly(c, shared, sess.id, sess.me.guid, || aid_send(c, shared, sess, sel, aids, out))?;
    Ok(true)
}

fn addon_outs(c: &Writer, shared: &Shared, sess: &Sess, pos: Option<Pos>, ready: &mut bool, outs: Vec<ovmod::Out>) -> io::Result<()> {
    for o in outs {
        match o {
            ovmod::Out::Addon(md, name, data) => {
                for b in ovmod::answer_bodies(sess.me.guid, &md, &name, &data) {
                    c.send_raw(ovmod::SMSG_MESSAGECHAT, &b)?;
                }
            }
            ovmod::Out::Say(t) => say(c, &t)?,
            ovmod::Out::Change(ch) => apply(c, shared, &ch)?,
            ovmod::Out::DenCmd(t) => {
                if let Some(p) = pos {
                    // a friend's panel button goes through the same rules as what they type (hosting.rs)
                    if sess.me.host {
                        chat(c, shared, sess, p, &format!(".den {t}"))?;
                    } else {
                        guest_chat(c, shared, sess, p, &format!(".den {t}"))?;
                    }
                }
            }
            ovmod::Out::Ask(what) => ask_launcher(c, shared, &what)?,
            ovmod::Out::Ready => {
                if !*ready {
                    *ready = true;
                    shared.event("builder_ready", &sess.me.name, "");
                }
            }
            ovmod::Out::Hello => {
                let held = shared.notices.lock().unwrap_or_else(|e| e.into_inner()).hello(sess.id);
                shared.event("den_ui", &sess.me.name, "");
                send_addon(c, sess.me.guid, "wuden", "status", &status_line(shared, sess))?;
                for it in held {
                    toast(c, shared, sess.me.guid, &it)?;
                }
            }
            ovmod::Out::Status => send_addon(c, sess.me.guid, "wuden", "status", &status_line(shared, sess))?,
            // the addon's own check (the den-test's `.den ui report`): kept only while the test's channel is on
            ovmod::Out::Report(t) => {
                if shared.cfg.test_control {
                    shared.event("ui_report", &sess.me.name, &t);
                    testctl::write_ui(&shared.cfg.den_dir, &t);
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- the Den bar (notice.rs): no chat spam

/// The Den bar's status line: your role, how many props, how long since the Den was last kept.
fn status_line(shared: &Shared, sess: &Sess) -> String {
    let role = if sess.me.host { "owner" } else if crate::hosting::may_build(shared, &sess.me) { "builder" } else { "visitor" };
    let props = shared.den.lock().unwrap_or_else(|e| e.into_inner()).props.len();
    let at = shared.autosaved_at.load(Ordering::Relaxed);
    let saved = if at == 0 { "-".to_string() } else { now().saturating_sub(at).to_string() };
    format!("role={role};props={props};saved={saved}")
}

fn send_addon(c: &Writer, guid: u64, md: &str, name: &str, data: &str) -> io::Result<()> {
    for b in ovmod::answer_bodies(guid, md, name, data) {
        c.send_raw(ovmod::SMSG_MESSAGECHAT, &b)?;
    }
    Ok(())
}

/// An addon line to one player (hosting.rs, tourney.rs: the Tournament window).
pub fn addon_to(c: &Writer, guid: u64, md: &str, name: &str, data: &str) -> io::Result<()> {
    send_addon(c, guid, md, name, data)
}

/// A system line to one player (hosting.rs, tourney.rs).
pub fn say_to(c: &Writer, text: &str) -> io::Result<()> {
    say(c, text)
}

/// One toast in the session's Den bar.
fn toast(c: &Writer, shared: &Shared, guid: u64, item: &Item) -> io::Result<()> {
    let Some(t) = &item.toast else { return Ok(()) };
    shared.event("notice", "", &format!("toast {}", item.kind));
    send_addon(c, guid, "wuden", "toast", &notice::toast_data(item.kind, t))
}

/// The old way, for a game without the bar: chat lines, her line, the middle of the screen.
fn old_way(c: &Writer, shared: &Shared, item: &Item) -> io::Result<()> {
    if !item.old.is_empty() {
        shared.event("notice", "", &format!("chat {}", item.kind));
    }
    for o in &item.old {
        match o {
            Old::Line(t) => say(c, t)?,
            Old::Jaina(t) => monster_say(c, t)?,
            Old::Middle(t) => c.send(&SMSG_NOTIFICATION { notification: t.clone() })?,
        }
    }
    Ok(())
}

/// Something nobody typed for, to one session: a toast in its Den bar, held until the bar's hello, or the old way.
fn notify(c: &Writer, shared: &Shared, sess: u64, guid: u64, item: Item) -> io::Result<()> {
    let route = shared.notices.lock().unwrap_or_else(|e| e.into_inner()).route(sess, &item);
    match route {
        Route::Toast => toast(c, shared, guid, &item),
        Route::Held => Ok(()),
        Route::Old => old_way(c, shared, &item),
    }
}

/// To the host in the world now (the launcher's answers, the companion, the autosave).
fn notify_host(shared: &Shared, item: Item) -> io::Result<()> {
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| (l.0, l.1.clone()));
    match live {
        Some((sess, w)) => notify(&w, shared, sess, PLAYER_GUID, item),
        None => Ok(()),
    }
}

/// Data for the session's Den bar only (nothing in the chat when it isn't there): `wuden <name> <data>`.
fn bar_data(c: &Writer, shared: &Shared, sess: u64, guid: u64, name: &str, data: &str) -> io::Result<()> {
    if shared.notices.lock().unwrap_or_else(|e| e.into_inner()).has_ui(sess) {
        send_addon(c, guid, "wuden", name, data)?;
    }
    Ok(())
}

/// `f`, with the system lines it says to `c` turned into toasts when the session has the Den bar (else said).
fn quietly(c: &Writer, shared: &Shared, sess: u64, guid: u64, f: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    if !shared.notices.lock().unwrap_or_else(|e| e.into_inner()).has_ui(sess) {
        return f();
    }
    let (r, lines) = notice::keeping(c.addr(), f);
    for l in lines.iter().take(3) {
        let warn = ["can't", "Only ", "Nothing", "Select ", "Pick ", "Usage", "couldn't", "doesn't"].iter().any(|w| l.contains(w));
        toast(c, shared, guid, &Item::line(if warn { "warn" } else { "info" }, l.clone()))?;
    }
    r
}

/// Twice a second (companion_ticker): a session whose bar didn't say hello in time is told the old way.
fn notices_due(shared: &Shared) {
    let due = shared.notices.lock().unwrap_or_else(|e| e.into_inner()).due(now_ms());
    for (sess, items) in due {
        let w = shared.players.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|p| p.sess == sess).map(|p| p.w.clone());
        if let Some(w) = w {
            for it in &items {
                let _ = old_way(&w, shared, it);
            }
        }
    }
}

/// Ask the launcher for an invite link or to open Forge (ovmod::ASK_FILE); its answer comes back through
/// `watch_open`. Without a launcher watching, the line says what to press there instead.
fn ask_launcher(c: &Writer, shared: &Shared, what: &str) -> io::Result<()> {
    ovmod::ask(&shared.cfg.den_dir, what);
    shared.event("ask", "", what);
    say(c, match what {
        "invite" => "Making an invite link for one friend... It shows in the Den bar (and the Den panel, /den) in a few seconds. (Or press Invite friends in the launcher.)",
        _ => "Opening Forge to shape your Den's ground... (Or press Shape the ground in the launcher.) Your game shows the new ground after a restart.",
    })
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
    [v.x, v.y, v.z, i.orientation].iter().all(|f| f.is_finite()).then_some(At { x: v.x, y: v.y, z: v.z, o: i.orientation })
}

/// A client movement packet read by hand: MSG_MOVE_START_FORWARD (0x0B5) .. SET_WALK_MODE (0x0C3), FALL_LAND ..
/// STOP_SWIM (0x0C9 - 0x0CB), SET_FACING / SET_PITCH (0x0DA, 0x0DB), HEARTBEAT (0x0EE) and, flying (the Dojo's
/// `.den fly`), CMSG_MOVE_SET_FLY (0x346), START_ASCEND / STOP_ASCEND (0x359, 0x35A) and START_DESCEND (0x3A7):
/// packed guid, u32 flags, u16 flags2, u32 time, then x, y, z, orientation.
pub fn raw_move(op: u32, body: &[u8]) -> Option<Pos> {
    if !matches!(op, 0x0B5..=0x0C3 | 0x0C9..=0x0CB | 0x0DA | 0x0DB | 0x0EE | 0x346 | 0x359 | 0x35A | 0x3A7) {
        return None;
    }
    let mask = *body.first()?;
    let at = 1 + mask.count_ones() as usize + 4 + 2 + 4;
    let f = |k: usize| body.get(at + 4 * k..at + 4 * k + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (x, y, z, o) = (f(0)?, f(1)?, f(2)?, f(3)?);
    [x, y, z, o].iter().all(|v| v.is_finite() && v.abs() < 1.0e6).then_some(At { x, y, z, o })
}

/// The Builder's guard rail (buildtools.rs `guard`, docs/DEN-TOOLS.md): a player who sank into a prop marked Solid
/// (its model has no collision, or the game hadn't drawn it yet) or fell through the Den's ground is put back on top
/// by a near teleport, at most once a second and a half each.
fn guard_rail(c: &Writer, shared: &Shared, sess: &Sess, was: Pos, n: Pos, op: u32, body: &[u8]) -> io::Result<()> {
    static LAST: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new());
    static COUNTER: AtomicU64 = AtomicU64::new(1_000_000);
    let flags = raw_move(op, body).and_then(|_| movewatch::raw_flags(body)).unwrap_or(0);
    let landed = op == movewatch::MSG_MOVE_FALL_LAND;
    let up = buildtools::guard(&shared.den.lock().unwrap_or_else(|e| e.into_inner()), [was.x, was.y, was.z], [n.x, n.y, n.z], flags, landed);
    let Some(z) = up else { return Ok(()) };
    let t = now_ms();
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.iter().any(|(s, at)| *s == sess.id && t.saturating_sub(*at) < 1500) {
            return Ok(());
        }
        last.retain(|(s, _)| *s != sess.id);
        last.push((sess.id, t));
    }
    shared.event("guard", &sess.me.name, &format!("{:.1} {:.1} {:.1} -> {:.1}", n.x, n.y, n.z, z));
    let counter = (COUNTER.fetch_add(1, Ordering::Relaxed) & 0xFFFF_FFFF) as u32;
    c.send_raw(places::MSG_MOVE_TELEPORT_ACK, &places::teleport_body(sess.me.guid, counter, movewatch::server_ms(), n.x, n.y, z, n.o))
}

/// A move from the game: what the watch makes of it into the events (movewatch.rs), and the host's copy kept for
/// `.den where`.
fn track(shared: &Shared, sess: &Sess, watch: &mut Option<movewatch::Watch>, op: u32, body: &[u8], n: Pos) {
    let Some(w) = watch.as_mut().filter(|w| w.sess == sess.id) else { return };
    let flags = raw_move(op, body).and_then(|_| movewatch::raw_flags(body)).unwrap_or(0);
    let ground = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(n.x, n.y);
    for (ev, detail) in w.moved(op, flags, n, ground) {
        shared.event(ev, &sess.me.name, &detail);
    }
    if sess.me.host {
        *shared.moves.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
    }
}

/// How many addons the client listed in CMSG_AUTH_SESSION (u32 size, then zlib: u32 count, the addons...).
pub fn addon_count(blob: &[u8]) -> usize {
    let b = play::addon_bytes(blob);
    if b.len() < 4 {
        return 0;
    }
    (u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).min(512)
}

/// The times of the game's own settings Den Lite keeps (accdata.rs; 0: none kept, the game uses its defaults).
fn account_times(shared: &Shared, account: &str, character: &str, mask: u32) -> SMSG_ACCOUNT_DATA_TIMES {
    let kept = crate::accdata::times(&shared.cfg.den_dir, account, character, mask);
    let mut m = CacheMask::default();
    for (i, d) in m.data_mut().iter_mut().enumerate() {
        if mask & (1 << i) != 0 {
            *d = Some(kept.get(i).copied().unwrap_or(0));
        }
    }
    SMSG_ACCOUNT_DATA_TIMES { unix_time: now() as u32, unknown1: 1, mask: m }
}

fn gender_of(g: HeroGender) -> Gender {
    match g {
        HeroGender::Male => Gender::Male,
        HeroGender::Female => Gender::Female,
    }
}

fn display_id(g: HeroGender) -> i32 {
    // CreatureDisplayInfo: the human male and female player models.
    match g {
        HeroGender::Male => 49,
        HeroGender::Female => 50,
    }
}

pub fn character(shared: &Shared, me: &Me) -> Character {
    let (spot, z, map, area) = den_where(shared);
    let empty = CharacterGear { equipment_display_id: 0, inventory_type: InventoryType::NonEquip, enchantment: 0 };
    let mut equipment = [empty; 23];
    for &(slot, _, display, inv) in OUTFIT {
        equipment[slot] = CharacterGear { equipment_display_id: display, inventory_type: inv, enchantment: 0 };
    }
    let (class, level) = if me.host { sandbox::class_of(shared) } else { (Class::Warrior, 1) };
    Character {
        guid: Guid::new(me.guid),
        name: me.name.clone(),
        race: Race::Human,
        class,
        gender: gender_of(me.gender),
        skin: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        facial_hair: 0,
        level: Level::new(level),
        area,
        map,
        position: Vector3d { x: spot.x, y: spot.y, z },
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

/// A player's create: for its own client (`own`: the self flag, it controls it) or for the others'.
fn player_create(me: &Me, p: Pos, own: bool) -> SMSG_UPDATE_OBJECT {
    player_create_as(me, p, own, None, None)
}

/// ... with the class picked (sandbox.rs: its class, level, health and power), for the host's own copy.
fn player_create_as(me: &Me, p: Pos, own: bool, kit: Option<&crate::classes::Kit>, looks: Option<&crate::gear::Gear>) -> SMSG_UPDATE_OBJECT {
    let mut b = UpdatePlayerBuilder::new()
        .set_object_guid(Guid::new(me.guid))
        .set_object_scale_x(1.0)
        .set_unit_bytes_0(Race::Human, Class::Warrior, gender_of(me.gender), Power::Rage)
        .set_unit_health(100)
        .set_unit_maxhealth(100)
        .set_unit_maxpower2(1000)
        .set_unit_level(1)
        .set_unit_factiontemplate(1)
        .set_unit_displayid(display_id(me.gender))
        .set_unit_nativedisplayid(display_id(me.gender))
        .set_unit_boundingradius(0.389)
        .set_unit_combatreach(1.5)
        .set_unit_mod_cast_speed(1.0)
        .set_unit_hoverheight(1.0)
        // UNIT_FLAG_PLAYER_CONTROLLED, UNIT_FLAG2_REGENERATE_POWER (Player::Create)
        .set_unit_flags(0x0000_0008)
        .set_unit_flags_2(0x0000_0800)
        .set_player_field_bytes(0, 0, 0, 0)
        .set_player_bytes_2(0, 0, 0, 2)
        .set_player_bytes_3(if me.gender == HeroGender::Female { 1 } else { 0 }, 0, 0, 0)
        .set_player_next_level_xp(400)
        .set_player_watched_faction_index(-1);
    if let Some(k) = kit {
        b = sandbox::with_class(b, k, gender_of(me.gender));
    }
    for &(slot, entry, _, _) in OUTFIT {
        let idx = VisibleItemIndex::try_from(slot as u8).expect("an equipment slot");
        b = b.set_player_visible_item(VisibleItem::new(entry, [0, 0]), idx);
    }
    // the looks picked with `.den gear` (gear.rs), every slot
    if let Some(g) = looks {
        b = sandbox::with_gear(b, g);
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
        // the server's clock, as AzerothCore's Unit::BuildMovementPacket (GameTime::GetGameTimeMS); was 0
        timestamp: movewatch::server_ms(),
        turn_rate: std::f32::consts::PI,
        walking_speed: 2.5,
    };
    SMSG_UPDATE_OBJECT {
        objects: vec![Object::CreateObject2 {
            guid3: Guid::new(me.guid),
            mask2: UpdateMask::Player(b.finalize()),
            // AzerothCore's flags: SELF (its own copy) | LIVING | STATIONARY_POSITION (movewatch::player_update_flags);
            // the crate writes the Living block for LIVING whatever else is set, as the game reads it
            movement2: MovementBlock {
                update_flag: MovementBlock_UpdateFlag::new(movewatch::player_update_flags(own), None, None, None, None, Some(living), None, None),
            },
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
fn enter_world(c: &Writer, shared: &Shared, account: &str, me: &Me, p: Pos) -> io::Result<()> {
    let (spot, z, map, area) = den_where(shared);
    c.send(&SMSG_LOGIN_VERIFY_WORLD { map, position: Vector3d { x: p.x, y: p.y, z: p.z }, orientation: p.o })?;
    c.send(&account_times(shared, account, &me.name, CacheMask::PER_CHARACTER_CACHE))?;
    c.send(&SMSG_FEATURE_SYSTEM_STATUS { complaint_status: ComplaintStatus::EnabledWithAutoIgnore, voice_chat_enabled: false })?;
    // no message of the day: it is a line in the chat (the Den bar or the welcome says hello instead)
    c.send(&SMSG_MOTD { motds: vec![] })?;
    c.send(&SMSG_BINDPOINTUPDATE { position: Vector3d { x: spot.x, y: spot.y, z }, map, area })?;
    // the class's spells and bar when one is picked (sandbox.rs), else none, as before
    sandbox::initial(c, shared, me)?;
    c.send(&SMSG_INITIALIZE_FACTIONS { factions: vec![FactionInitializer { flag: FactionFlag::empty(), standing: 0 }; 128] })?;
    c.send(&SMSG_LOGIN_SETTIMESPEED { datetime: login_time(), timescale: 1.0 / 60.0, unknown1: 0 })?;
    // the player (Map::AddPlayerToMap), THEN the time sync (Player::SendInitialPacketsAfterAddToMap:
    // ResetTimeSync, SendTimeSync), as AzerothCore and the message crate's own wrath_server example do. Den Lite sent
    // the time sync before the player existed, and a control update during the loading screen that AzerothCore never
    // sends at login (it is said when the game takes its character: CMSG_SET_ACTIVE_MOVER, in `session`).
    let looks = if me.host { shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone() } else { None };
    c.send(&player_create_as(me, p, true, sandbox::kit_for(shared, me).as_ref(), looks.as_ref()))?;
    c.send_raw(movewatch::SMSG_TIME_SYNC_REQ, &movewatch::time_sync_body(0))?;
    sandbox::after_create(c, shared, me)?;
    c.send(&SMSG_INIT_WORLD_STATES { map, area, sub_area: area, states: vec![] })?;
    let objs = shared.den.lk().everything();
    send_creates(c, shared, &objs)?;
    Ok(())
}

/// `.den go` (places.rs): the host's character to another spot. The same map: a near teleport (the game moves its
/// character and answers); another map: a far one (its loading screen; `arrive_far` once it answers). Nothing moves
/// on Den Lite's side until the game answers (the session's teleport acks).
fn travel(c: &Writer, shared: &Shared, sess: &Sess, t: places::Target) -> io::Result<()> {
    static TRIPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    if Map::try_from(t.map).is_err() {
        return say(c, &format!("Your game has no map {}.", t.map));
    }
    let from = map_of(shared, c).unwrap_or_else(|| den_map(shared));
    let far = !places::is_near(from, t.map);
    let counter = TRIPS.fetch_add(1, Ordering::Relaxed);
    // where the player was, for `.back` (places.rs): named by the nearest place
    if let Some(at) = where_now(shared, sess) {
        let title = places::near(from, at.x, at.y).map(|n| n.title.to_string()).unwrap_or_else(|| format!("map {from} at {:.0}, {:.0}", at.x, at.y));
        places::left(sess.id, places::Spot { map: from, x: at.x, y: at.y, z: at.z, o: at.o, title });
    }
    let trip = places::Trip { map: t.map, x: t.x, y: t.y, z: t.z, o: t.o, far, counter, area: t.area, title: t.title.clone() };
    shared.trips.lock().unwrap_or_else(|e| e.into_inner()).insert(sess.id, trip);
    shared.event("travel", &sess.me.name, &format!("{} map {} {:.1} {:.1} {:.1}", if far { "far" } else { "near" }, t.map, t.x, t.y, t.z));
    if far {
        // the game drops every object it knows on a new map: the companion and the Dojo's build go quietly
        if sess.me.host {
            shared.companion.lock().unwrap_or_else(|e| e.into_inner()).away();
            shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job = None;
        }
        // the others on the old map lose this player
        for o in others_here(shared, sess.id, c) {
            let _ = o.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(sess.me.guid), target_died: false });
        }
        c.send_raw(places::SMSG_TRANSFER_PENDING, &places::transfer_pending_body(t.map))?;
        c.send_raw(places::SMSG_NEW_WORLD, &places::new_world_body(t.map, t.x, t.y, t.z, t.o))?;
    } else {
        c.send_raw(places::MSG_MOVE_TELEPORT_ACK, &places::teleport_body(sess.me.guid, counter, movewatch::server_ms(), t.x, t.y, t.z, t.o))?;
    }
    say(c, &format!("Off to {}...", t.title))
}

/// Where a player stands now (the players list; None before they are in the world).
fn where_now(shared: &Shared, sess: &Sess) -> Option<Pos> {
    shared.players.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|x| x.sess == sess.id).map(|x| x.pos)
}

/// Your Den's spot as a trip's end (`.den home`, `.den go den`, the Teleports window's Back to my Den): from any map.
fn home_target(shared: &Shared) -> places::Target {
    let (spot, z, _, _) = den_where(shared);
    let title = shared.den.lock().unwrap_or_else(|e| e.into_inner()).place.as_ref().map(|p| format!("your Den ({})", p.0)).unwrap_or_else(|| "your Den".into());
    places::Target { map: spot.map, x: spot.x, y: spot.y, z: z + places::TYPED_LIFT, o: 0.0, area: None, title }
}

/// `.back`: the spot before the last teleport (places.rs `back`); none kept: your Den.
fn go_back(c: &Writer, shared: &Shared, sess: &Sess) -> io::Result<()> {
    match places::back(sess.id) {
        Some(s) => travel(c, shared, sess, places::Target { map: s.map, x: s.x, y: s.y, z: s.z + places::TYPED_LIFT, o: s.o, area: None, title: s.title }),
        None => travel(c, shared, sess, home_target(shared)),
    }
}

/// A fall through the world after a teleport (places.rs Landing): put back over the last ground, flying, by a near
/// teleport the game answers like any other (its trip is titled [`places::LIFTED`], so the landing keeps its count).
fn lift(c: &Writer, shared: &Shared, sess: &Sess, x: f32, y: f32, z: f32) -> io::Result<()> {
    static LIFTS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1_000_000);
    let map = map_of(shared, c).unwrap_or_else(|| den_map(shared));
    let counter = LIFTS.fetch_add(1, Ordering::Relaxed);
    let trip = places::Trip { map, x, y, z, o: 0.0, far: false, counter, area: None, title: places::LIFTED.into() };
    shared.trips.lock().unwrap_or_else(|e| e.into_inner()).insert(sess.id, trip);
    shared.event("lifted", &sess.me.name, &format!("map {map} {x:.1} {y:.1} {z:.1}"));
    let flying = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).fly;
    if !flying {
        fly(c, shared, sess, "on")?;
    }
    c.send_raw(places::MSG_MOVE_TELEPORT_ACK, &places::teleport_body(sess.me.guid, counter, movewatch::server_ms(), x, y, z, 0.0))?;
    say(c, "You fell through the ground there: you are back over it, flying. Hold X to come down onto it (.den fly off lands you), or .back.")
}

/// Every move after a teleport: a fall through the world is lifted back (three times, higher each time), then the
/// player goes back where they came from.
fn watch_landing(c: &Writer, shared: &Shared, sess: &Sess, landing: &mut Option<places::Landing>, body: &[u8], n: Pos) -> io::Result<()> {
    let Some(l) = landing.as_mut() else { return Ok(()) };
    let map = map_of(shared, c).unwrap_or_else(|| den_map(shared));
    let flags = movewatch::raw_flags(body).unwrap_or(0);
    match l.check(map, n.x, n.y, n.z, flags) {
        None => Ok(()),
        Some(places::Rescue::Lift(x, y, z)) => lift(c, shared, sess, x, y, z),
        Some(places::Rescue::Back) => {
            *landing = None;
            shared.event("lifted", &sess.me.name, "back");
            say(c, "The ground there didn't hold you: taking you back.")?;
            let r = go_back(c, shared, sess);
            // the spot under the world that trip just kept for `.back` isn't one to go back to
            let _ = places::back(sess.id);
            r
        }
    }
}

/// After a far teleport (a new world: the game forgot them): flying and the speed as they were.
fn moves_again(c: &Writer, shared: &Shared, sess: &Sess) -> io::Result<()> {
    let (fly_on, speed, a, b, f) = {
        let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
        (d.fly, d.speed_now(), d.counter(), d.counter(), d.counter())
    };
    if fly_on {
        c.send_raw(dojo::SMSG_MOVE_SET_CAN_FLY, &dojo::can_fly_body(sess.me.guid, f))?;
    }
    if speed > 1 {
        c.send_raw(dojo::SMSG_FORCE_RUN_SPEED_CHANGE, &dojo::run_speed_body(sess.me.guid, a, dojo::BASE_RUN * speed as f32))?;
        c.send_raw(dojo::SMSG_FORCE_FLIGHT_SPEED_CHANGE, &dojo::flight_speed_body(sess.me.guid, b, dojo::BASE_FLIGHT * speed as f32))?;
    }
    Ok(())
}

/// The Teleports window's line (Teleports.lua, the Den bar only): `wuden places` `den=<your Den's place>;map=<the map
/// you are on>;back=<where .back goes|->;kept=<your Dens kept at places, comma separated>`.
fn places_line(c: &Writer, shared: &Shared, sess: &Sess) -> io::Result<()> {
    let clean = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric() || " '-".contains(*c)).take(40).collect::<String>();
    let den = shared.den.lock().unwrap_or_else(|e| e.into_inner()).place.as_ref().map(|p| clean(p.0.as_str())).unwrap_or_else(|| "the island".into());
    let map = map_of(shared, c).unwrap_or_else(|| den_map(shared));
    let back = places::peek(sess.id).map(|s| clean(s.title.as_str())).unwrap_or_else(|| "-".into());
    let kept: Vec<String> = kept_places(shared).into_iter().map(|k| clean(if k.1.is_empty() { k.0.as_str() } else { k.1.as_str() })).take(12).collect();
    bar_data(c, shared, sess.id, sess.me.guid, "places", &format!("den={den};map={map};back={back};kept={}", kept.join(",")))
}

/// After a far `.den go`, once the game has the new map loaded (MSG_MOVE_WORLDPORT_ACK), in AzerothCore's order
/// (`WorldSession::HandleMoveWorldportAck`: SendInitialPacketsBeforeAddToMap, the player added to the map,
/// SendInitialPacketsAfterAddToMap): the bind point, spells, factions, the clock, the player's own create (the self
/// flags 0x61, the server's clock: movewatch.rs, as at login), the time sync, the world states; then the Den and its
/// dummies when they are on this map, and the friends on it.
fn arrive_far(c: &Writer, shared: &Shared, sess: &Sess, t: &places::Trip) -> io::Result<()> {
    let me = &sess.me;
    let at = At { x: t.x, y: t.y, z: t.z, o: t.o };
    set_map(shared, c, t.map, !me.host);
    let (spot, z, home_map, home_area) = den_where(shared);
    let area = t.area.map(warea).unwrap_or(if t.map == spot.map { home_area } else { AREA });
    c.send(&SMSG_BINDPOINTUPDATE { position: Vector3d { x: spot.x, y: spot.y, z }, map: home_map, area: home_area })?;
    sandbox::initial(c, shared, me)?;
    c.send(&SMSG_INITIALIZE_FACTIONS { factions: vec![FactionInitializer { flag: FactionFlag::empty(), standing: 0 }; 128] })?;
    c.send(&SMSG_LOGIN_SETTIMESPEED { datetime: login_time(), timescale: 1.0 / 60.0, unknown1: 0 })?;
    let looks = if me.host { shared.gear.lock().unwrap_or_else(|e| e.into_inner()).clone() } else { None };
    c.send(&player_create_as(me, at, true, sandbox::kit_for(shared, me).as_ref(), looks.as_ref()))?;
    c.send_raw(movewatch::SMSG_TIME_SYNC_REQ, &movewatch::time_sync_body(0))?;
    sandbox::after_create(c, shared, me)?;
    c.send(&SMSG_INIT_WORLD_STATES { map: wmap(t.map), area, sub_area: area, states: vec![] })?;
    if t.map == spot.map {
        let objs = shared.den.lock().unwrap_or_else(|e| e.into_inner()).everything();
        send_creates(c, shared, &objs)?;
        let dummies = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).dummies.clone();
        for d in &dummies {
            c.send(&dummy_create(shared, d))?;
        }
    }
    for o in others_here(shared, sess.id, c) {
        c.send(&player_create(&o.me, o.pos, false))?;
        let _ = o.w.send(&player_create(me, at, false));
    }
    Ok(())
}

/// The Dens kept at places (`<dens>/places`, den.rs `relocate`): (file name, place name, spot).
fn kept_places(shared: &Shared) -> Vec<(String, String, den::Spot)> {
    let dir = shared.cfg.den_dir.join(places::DIR);
    den::list_names(&dir)
        .into_iter()
        .filter_map(|n| {
            let doc = den::read_file(&dir.join(format!("{n}.den.json"))).ok()?;
            match den::place_of(&doc) {
                Some((spot, name, _)) => Some((n, name, spot)),
                // the first Den (the island) is kept without a place of its own
                None => {
                    let home = den::Spot::default();
                    (n == places::key(home.map, home.x, home.y)).then(|| (n, "your first Den".to_string(), home))
                }
            }
        })
        .collect()
}

/// What a player is told on entering (after enter_world): the welcome, the first visit's cheer and the companion's
/// one greeting. Nothing in the chat: they wait for the Den bar's hello and are toasts there (notice.rs); a game
/// without the bar gets the old lines (the welcome and the quick start) after `Config::ui_wait_ms`.
fn arrive(c: &Writer, shared: &Shared, sess: &Sess, p: Pos) -> io::Result<()> {
    let me = &sess.me;
    shared.notices.lock().unwrap_or_else(|e| e.into_inner()).entered(sess.id, now_ms() + shared.cfg.ui_load_ms);
    if !me.host {
        let host = shared.cfg.hero.name.clone();
        let builds = crate::hosting::may_build(shared, me);
        return notify(c, shared, sess.id, me.guid, Item {
            kind: "info",
            toast: Some(format!("Welcome to {host}'s Den. What you say here, everyone in the Den hears.")),
            old: vec![
                Old::Line(format!("Welcome to {host}'s Den. Say hello: what you say here, everyone in the Den hears.")),
                Old::Line(if builds { "You can build here too: .den place chair, .den stamp inn now, .build undo." }
                    else { "Your host builds here; .den find lamp shows what there is to place." }.into()),
            ],
        });
    }
    let n = shared.den.lk().props.len();
    let first = cheer_line(shared, "first-visit");
    let mut old = vec![Old::Line(format!("Welcome to your Den: {n} props. {BUILDER_HINT}"))];
    old.extend(TUTORIAL.iter().map(|l| Old::Line(l.to_string())));
    old.extend(first.map(|l| Old::Middle(l.into())));
    let toast = first.map(String::from).unwrap_or_else(|| if n == 0 { "Welcome back to your Den.".into() } else { format!("Welcome back: {n} props, as you left them.") });
    notify(c, shared, sess.id, me.guid, Item { kind: if first.is_some() { "cheer" } else { "info" }, toast: Some(toast), old })?;
    // the companion: at your side from the start (Config::companion), one greeting, then quiet until spoken to
    let mut comp = shared.companion.lk();
    comp.arrive(now());
    let inside = shared.den.lk().inside(p.x, p.y, p.z);
    if shared.cfg.companion && inside && !comp.out {
        let at = comp.come(p, now_ms());
        drop(comp);
        c.send(&companion_create(at))?;
        notify(c, shared, sess.id, me.guid, Item::jaina(companion::GREET))?;
        beat_now(shared);
    }
    Ok(())
}

/// What the tips look at (companion::TipCtx): the Den now.
fn tip_ctx(shared: &Shared) -> companion::TipCtx {
    let (props, size) = { let d = shared.den.lk(); (d.props.len(), d.ring() + 1) };
    let saved = den::list_names(&shared.cfg.den_dir).iter().any(|n| n != "backup" && n != den::AUTOSAVE);
    companion::TipCtx { props, saved, size }
}

/// "Jaina, stay" and the other short commands (companion::command), and `.den companion stay|follow|come|tips`.
fn order(c: &Writer, shared: &Shared, p: Pos, cmd: companion::Cmd) -> io::Result<()> {
    use companion::Cmd;
    let (ms, t) = (now_ms(), now());
    if cmd == Cmd::Help {
        let ctx = tip_ctx(shared);
        let mut comp = shared.companion.lk();
        comp.heard_now(t);
        let tip = comp.tip(ctx);
        let out = comp.out;
        drop(comp);
        return if out { monster_say(c, tip) } else { say(c, tip) };
    }
    let mut comp = shared.companion.lk();
    if !comp.out {
        drop(comp);
        return say(c, "Your companion isn't here: .den companion calls her.");
    }
    comp.heard_now(t);
    match cmd {
        Cmd::Stay => {
            let here = comp.stay(ms);
            drop(comp);
            // she stops where she is (a walk of nothing ends the one she was on)
            walk(c, here, here, 100, shared.companion.lk().moves, &[])?;
            monster_say(c, companion::STAYING)
        }
        Cmd::Follow => {
            comp.follow_on();
            let step = comp.follow(p, ms);
            let hop = comp.take_hop();
            let (id, via) = (comp.moves, comp.via.clone());
            drop(comp);
            if let Some((from, to, d)) = step {
                step_send(c, from, to, d, id, &via, hop)?;
            }
            monster_say(c, companion::FOLLOWING)
        }
        Cmd::Come => {
            let step = comp.come_walk(p, ms);
            let (id, via) = (comp.moves, comp.via.clone());
            drop(comp);
            if let Some((from, to, d)) = step {
                walk(c, from, to, d, id, &via)?;
            }
            monster_say(c, companion::COMING)
        }
        Cmd::Away => {
            let left = comp.brain.leftovers();
            comp.dismiss();
            drop(comp);
            monster_say(c, companion::BYE)?;
            c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(companion::GUID), target_died: false })?;
            for g in left {
                c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false })?;
            }
            beat_now(shared);
            Ok(())
        }
        Cmd::Help => Ok(()),
    }
}

/// An encouragement line, once ever (cheers.rs) and rarely (notice::cheer_ok: one that comes too soon waits for a
/// later moment of its kind): the line, now marked said.
fn cheer_line(shared: &Shared, key: &str) -> Option<&'static str> {
    crate::cheers::unsaid(&shared.cfg.den_dir, key)?;
    if !notice::cheer_ok(&mut shared.last_cheer.lock().unwrap_or_else(|e| e.into_inner()), now(), shared.cfg.cheer_gap) {
        return None;
    }
    crate::cheers::once(&shared.cfg.den_dir, key)
}

/// An encouragement moment: a toast in the Den bar, or the middle of the screen; never in the chat frame.
fn cheer(c: &Writer, shared: &Shared, sess: &Sess, key: &str) -> io::Result<()> {
    match cheer_line(shared, key) {
        Some(line) => notify(c, shared, sess.id, sess.me.guid, Item::cheer(line)),
        None => Ok(()),
    }
}

/// The first-minute tutorial, said at every login (five lines: build, a stamp, undo, save, grow and the companion).
const TUTORIAL: &[&str] = &[
    "Quick start: 1) .den place chair  puts a chair in front of you (.den find lamp: what's there to place)",
    "2) .den stamp inn now  puts down a whole inn (.den stamps: the others)",
    "3) .build undo  takes the last one back",
    "4) .den save mine  keeps your Den in a file; the launcher's Saved Dens opens it again",
    "5) .den grow  makes the room bigger. Your companion helps: say \"Jaina, tips\", \"Jaina, stay\" or \"Jaina, follow\" (.den companion calls her)",
];

/// Gameobject creates, each shown as the editor's mode says (gizmo.rs `shown`: in build mode props are goobers the
/// client highlights and lets you right-click).
fn send_creates(c: &Writer, shared: &Shared, objs: &[WorldObject]) -> io::Result<()> {
    // only to a game on the Den's map (in a city elsewhere they would stand at these numbers there)
    if map_of(shared, c).is_some_and(|m| m != den_map(shared)) {
        return Ok(());
    }
    // a friend's game hasn't the player's own models (mymodels.rs): a stand-in in their place
    let stand_ins: Vec<WorldObject>;
    let objs = if is_guest(shared, c) {
        stand_ins = objs.iter().map(mymodels::stand_in).collect();
        &stand_ins[..]
    } else {
        objs
    };
    let build = shared.build_look.load(Ordering::Relaxed);
    for body in raw::gameobject_creates_shown(objs, PER_UPDATE, |o| gizmo::shown(o, build)) {
        c.send_raw(raw::SMSG_UPDATE_OBJECT, &body)?;
    }
    Ok(())
}

/// A change to the Den: its objects to everyone in the world (the one who made it, then the others), the line to
/// the one who made it.
fn apply(c: &Writer, shared: &Shared, ch: &Change) -> io::Result<()> {
    if !ch.created.is_empty() || !ch.destroyed.is_empty() {
        shared.event("den_change", "", &format!("+{} -{}", ch.created.len(), ch.destroyed.len()));
        testctl::note_change(ch.created.len(), ch.destroyed.len());
        jaina_saw(shared, ch);
    }
    let to_all = |w: &Writer| -> io::Result<()> {
        for &g in &ch.destroyed {
            w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false })?;
        }
        send_creates(w, shared, &ch.created)
    };
    to_all(c)?;
    for p in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        if !Arc::ptr_eq(&p.w.0, &c.0) {
            let _ = to_all(&p.w);
        }
    }
    if !ch.message.is_empty() {
        say(c, &ch.message)?;
    }
    Ok(())
}

/// A system line in the chat frame (or, while Den Lite answers the Den bar, a toast there: notice::keeping).
fn say(c: &Writer, text: &str) -> io::Result<()> {
    testctl::note_said(text);
    if notice::kept(c.addr(), text) {
        return Ok(());
    }
    c.send(&SMSG_MESSAGECHAT {
        chat_type: SMSG_MESSAGECHAT_ChatType::System { target6: Guid::new(0) },
        language: Language::Universal,
        sender: Guid::new(0),
        flags: 0,
        message: text.into(),
        tag: PlayerChatTag::None,
    })
}

/// The in-game tools, at every login of the host (the Leyline UI's Builder and Den panel talk to Den Lite: ovmod.rs).
pub const BUILDER_HINT: &str = "Builder: F7 or /builder (place, move, turn, delete, undo). Den panel: /den (Invite a friend, Shape the ground, save and load). Or type .den invite / .den forge.";

const HELP: &[&str] = &[
    ".den place <name>: one piece in front of you (chair, lantern, house...); .den find <word>: what there is",
    ".den stamps: the stamps; .den stamp <name> now [degrees]: put one down where you stand",
    ".build undo: take the last change back",
    ".den save <name> / .den load <name> [replace|add] / .den files: your Den in a file",
    ".den room white|stone|wood: the room's look; .den grow / .den size [1-4]: how big it is",
    ".den companion [stay|follow|come|tips|away]: your companion (or say \"Jaina, stay\"); .den ask <words> or \"Jaina, ...\" near her: to your assistant (Leyline's Agents page), or her own tips",
    ".den mode [id]: Leyline's game modes and how to try one on this PC; .den mods: the addons and mods in your game",
    ".den dojo: a training ring with dummies (your companion builds it); .den fly on|off; .den speed 1-5; \"Jaina, build a garden\" (.den dojo help: more)",
    ".den class <class> [level]: any class at any level, every spell it learns (.den class: the list); the dummies take your hits",
    ".den gear <item number>|<slot> <item>|list <slot>|reset: any item's look on you (looks: thunderfury, warglaives...)",
    ".den mounts [list|off]: every mount in your spellbook; flying ones fly with .den fly on",
    ".go <place> (stormwind, goldshire, icc, nagrand arena...) or .go <map> <x> <y> <z>: go anywhere your game has; .back: where you were; .den home: your Den, from anywhere; .den places [word]: the list; /tp: the Teleports window",
    ".den here: make your Den here (each place keeps its own); .den size city: a whole-city Den; .den copy <place>: a place from your own game files",
];

fn chat(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, text: &str) -> io::Result<()> {
    let words: Vec<String> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    // `.dojo ...` is `.den dojo ...` (the full Den's ov_dojo.lua has both)
    if w(0) == ".dojo" {
        return chat(c, shared, sess, p, &format!(".den dojo {}", words[1..].join(" ")));
    }
    // the GM ways to travel (places.rs; owner 4 Nov: "GM-style commands"): `.go`, `.tele`, `.teleport` and `.tp` are
    // `.den go`; `.back` and `.home` are `.den back` and `.den home`
    if matches!(w(0), ".go" | ".tele" | ".teleport" | ".tp") {
        return chat(c, shared, sess, p, &format!(".den go {}", words[1..].join(" ")));
    }
    if matches!(w(0), ".back" | ".home") {
        return chat(c, shared, sess, p, &format!(".den {}", w(0).trim_start_matches('.')));
    }
    match (w(0), w(1)) {
        // `.den help`: the Leyline UI's Den guide opens (a paged help window: Guide.lua) when the game has the UI;
        // the commands in the chat as well without it, and always for `.den` alone or `.den commands`
        (".den", "help") | (".den", "guide") if shared.notices.lock().unwrap_or_else(|e| e.into_inner()).has_ui(sess.id) => {
            send_addon(c, sess.me.guid, "wuden", "guide", "open")?;
            say(c, "The Den guide is open (Guide on the Den bar). .den commands lists the commands.")
        }
        (".den", "") | (".den", "help") | (".den", "guide") | (".den", "commands") => {
            for l in HELP {
                say(c, l)?;
            }
            Ok(())
        }
        // where you stand against the ground, and what the game did with its character (movewatch.rs; the den-test asks)
        (".den", "where") => {
            let (g, file) = { let d = shared.den.lk(); (d.floor_at(p.x, p.y), d.ground_here().is_some()) };
            let moves = shared.moves.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|w| w.sess == sess.id).map(|w| (w.summary(), w.ops_line()));
            let (moves, ops) = moves.unwrap_or_else(|| ("Nothing seen from your game yet".into(), "nothing".into()));
            say(c, &format!(
                "You stand at {:.1}, {:.1}, {:.2}: {} (the ground {:.2}{}). {moves}. Your game sent: {ops}.",
                p.x, p.y, p.z, movewatch::stand(p.z, g), g, if file { "" } else { ", the fixed floor" },
            ))
        }
        // the den-test's before / after: the Leyline UI's view of the character (WrathUnleashedUI W.DenDiag), into the
        // events as `addon` lines (wudiag ...)
        (".den", "diag") => {
            let tag: String = w(2).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(24).collect();
            let tag = if tag.is_empty() { "now".to_string() } else { tag };
            let ui = shared.notices.lock().unwrap_or_else(|e| e.into_inner()).has_ui(sess.id);
            if ui {
                send_addon(c, sess.me.guid, "wuden", "diag", &tag)?;
            }
            let ops = shared.moves.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|w| w.sess == sess.id).map(|w| w.ops_line());
            say(c, &format!("Diag {tag}: {}; your game sent: {}.", if ui { "asked the Leyline UI" } else { "no Leyline UI to ask" }, ops.unwrap_or_else(|| "nothing yet".into())))
        }
        // the den-test's feature check (tools/den-check featuretest): what the editor does when clicked, asked of the
        // Leyline UI over its own channel (`wuden ui <action>`, DenBar.lua D.TestAction). Only with the test's channel.
        (".den", "ui") if shared.cfg.test_control => {
            let rest: String = text.split_whitespace().skip(2).collect::<Vec<_>>().join(" ");
            send_addon(c, sess.me.guid, "wuden", "ui", &rest)
        }
        (".den", "stamps") => {
            let names = den::stamp_names(&shared.cfg.stamps_dir);
            say(c, &format!("Stamps: {}.", if names.is_empty() { "none here".into() } else { names.join(", ") }))
        }
        (".den", "stamp") => {
            if w(2).is_empty() {
                return say(c, "Which stamp? .den stamps lists them.");
            }
            // the Den home's staged stamp (DenHome.lua: mark, turn, here, place, cancel): Den Lite puts the stamp down
            // when it is marked, so turn and here put it down again (one undo each way), place keeps it, cancel takes it back
            if matches!(w(2), "place" | "cancel" | "turn" | "here") {
                let last = shared.last_stamp.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let Some((name, x, y, z, o, turn)) = last else { return say(c, "Mark a stamp first (Den home > Stamps).") };
                if w(2) == "place" {
                    *shared.last_stamp.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    return say(c, "The stamp is down. .build undo takes it back.");
                }
                let ch = shared.den.lk().undo();
                apply(c, shared, &ch)?;
                if w(2) == "cancel" {
                    *shared.last_stamp.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    return Ok(());
                }
                let add = if w(2) == "turn" { w(3).parse::<f32>().ok().filter(|d| d.is_finite() && d.abs() <= 360.0).unwrap_or(15.0) } else { 0.0 };
                let (x, y, z, o) = if w(2) == "here" { (p.x, p.y, p.z, p.o) } else { (x, y, z, o) };
                let turn = turn + add;
                return match den::read_stamp(&shared.cfg.stamps_dir, &name) {
                    Err(e) => say(c, &e),
                    Ok(st) => {
                        let ch = shared.den.lk().stamp(&st, x, y, z, o + turn.to_radians());
                        *shared.last_stamp.lock().unwrap_or_else(|e| e.into_inner()) = Some((name, x, y, z, o, turn));
                        apply(c, shared, &ch)
                    }
                };
            }
            let deg = words.iter().skip(3).find_map(|x| x.parse::<f32>().ok()).unwrap_or(0.0);
            if !deg.is_finite() || deg.abs() > 360.0 {
                return say(c, "Turn by up to 360 degrees.");
            }
            match den::read_stamp(&shared.cfg.stamps_dir, w(2)) {
                Err(e) => say(c, &e),
                Ok(st) => {
                    // all of it inside the Den: moved toward the middle when the player stands near its edge
                    let o = p.o + deg.to_radians();
                    let (ch, fx, fy) = {
                        let mut d = shared.den.lk();
                        let (fx, fy, _) = d.fit_stamp(&st, p.x, p.y, o);
                        (d.stamp(&st, fx, fy, p.z, o), fx, fy)
                    };
                    if !ch.created.is_empty() {
                        *shared.last_stamp.lock().unwrap_or_else(|e| e.into_inner()) = Some((w(2).to_string(), fx, fy, p.z, p.o, deg));
                    }
                    apply(c, shared, &ch)?;
                    if ch.created.is_empty() { Ok(()) } else { cheer(c, shared, sess, "first-piece") }
                }
            }
        }
        (".den", "find") => {
            let q = words[2..].join(" ");
            if q.is_empty() {
                return say(c, "Find what? .den find chair");
            }
            let hits = den::find_objects(&q);
            if hits.is_empty() {
                return say(c, &format!("Nothing called '{q}' here."));
            }
            let names: Vec<&str> = hits.iter().take(12).map(|h| h.3).collect();
            let more = if hits.len() > 12 { format!(" (and {} more)", hits.len() - 12) } else { String::new() };
            say(c, &format!("{}{more}. .den place <name> puts one down.", names.join(", ")))
        }
        (".den", "place") => {
            let q = words[2..].join(" ");
            if q.is_empty() {
                return say(c, "Place what? .den place chair (.den find <word> lists them)");
            }
            let Some(obj) = den::find_objects(&q).into_iter().next() else {
                return say(c, &format!("Nothing called '{q}' here. .den find <word> lists what there is."));
            };
            let st = den::one_piece(obj);
            let ch = shared.den.lk().stamp(&st, p.x, p.y, p.z, p.o);
            apply(c, shared, &ch)?;
            if ch.created.is_empty() { Ok(()) } else { cheer(c, shared, sess, "first-piece") }
        }
        (".build", "undo") => {
            let ch = shared.den.lk().undo();
            apply(c, shared, &ch)
        }
        (".den", "save") => {
            let r = shared.den.lk().save(&shared.cfg.den_dir, w(2), now());
            match r {
                Ok((path, n)) => {
                    shared.event("den_save", "", w(2));
                    say(c, &format!("Saved to a file ({n} props): {}", path.display()))?;
                    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("").trim_end_matches(".den.json").to_string();
                    bar_data(c, shared, sess.id, sess.me.guid, "saved", &name)?;
                    jaina_feed(shared, crate::jaina::Event::Saved);
                    cheer(c, shared, sess, "first-save")
                }
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
            let mut d = shared.den.lk();
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
            shared.event("den_load", "", &format!("{name} {}", if replace { "replace" } else { "add" }));
            apply(c, shared, &ch)
        }
        (".den", "room") => match den::look(w(2)) {
            None => say(c, "Looks: white, stone, wood."),
            Some(l) => {
                let mut d = shared.den.lk();
                d.look = l;
                let destroyed = d.build_room();
                let open = if d.ring() >= den::ROOM_RINGS && l.name != "white" { " (a city-size Den is open ground: .den size 4 or smaller for its floor and walls)" } else { "" };
                let ch = Change { created: d.room.clone(), destroyed, skipped: 0, message: format!("The room is {} now.{open}", l.name) };
                drop(d);
                apply(c, shared, &ch)
            }
        },
        // Den anywhere (places.rs, owner 3 Nov): go to a place, make the Den there, the list
        (".den", "go") | (".den", "goto") | (".den", "tp") | (".den", "home") => {
            let args: Vec<&str> = words[2..].iter().map(String::as_str).collect();
            if w(1) == "home" || matches!(args.as_slice(), ["den"] | ["home"] | ["my", "den"] | ["mine"]) {
                return travel(c, shared, sess, home_target(shared));
            }
            if matches!(args.as_slice(), ["back"]) {
                return go_back(c, shared, sess);
            }
            let here = map_of(shared, c).unwrap_or_else(|| den_map(shared));
            match places::parse_go_on(&args, Some(here)) {
                Ok(t) => travel(c, shared, sess, t),
                Err(e) => {
                    // one of your own Dens kept at a place, by its name
                    let q: String = args.join(" ").to_lowercase();
                    let kept = kept_places(shared).into_iter().find(|k| !q.is_empty() && (k.1.to_lowercase() == q || k.0 == q));
                    match kept {
                        Some((_, name, spot)) => travel(c, shared, sess, places::Target {
                            map: spot.map, x: spot.x, y: spot.y, z: spot.z + places::TYPED_LIFT, o: 0.0, area: None, title: format!("your Den at {name}"),
                        }),
                        None => say(c, &e),
                    }
                }
            }
        }
        (".den", "back") => go_back(c, shared, sess),
        // the Teleports window asks for its line (Teleports.lua)
        (".den", "places") if w(2) == "status" => places_line(c, shared, sess),
        (".den", "places") | (".den", "find-place") => {
            let q = words[2..].join(" ");
            for l in places::list_lines(Some(q.as_str()).filter(|q| !q.is_empty())) {
                say(c, &l)?;
            }
            if !q.is_empty() {
                return Ok(());
            }
            let kept: Vec<String> = kept_places(shared).into_iter().map(|k| if k.1.is_empty() { k.0 } else { k.1 }).collect();
            let now_at = shared.den.lock().unwrap_or_else(|e| e.into_inner()).place.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| "the island".into());
            say(c, &format!("Your Den is at {now_at}.{} .den go <place>, then .den here.", if kept.is_empty() { String::new() } else { format!(" Dens you kept at other places: {}.", kept.join(", ")) }))
        }
        (".den", "here") => {
            let map = map_of(shared, c).unwrap_or_else(|| den_map(shared));
            let near = places::near(map, p.x, p.y);
            let typed: String = text.split_whitespace().skip(2).collect::<Vec<_>>().join(" ").chars().filter(|c| c.is_ascii_alphanumeric() || " '-".contains(*c)).take(40).collect();
            let name = if !typed.trim().is_empty() { typed.trim().to_string() } else { near.map(|n| n.title.to_string()).unwrap_or_else(|| format!("map {map} at {:.0}, {:.0}", p.x, p.y)) };
            let area = near.map(|n| n.area).unwrap_or(0);
            let spot = den::Spot { map, x: p.x, y: p.y, z: p.z, half: 0.0 };
            let ch = shared.den.lock().unwrap_or_else(|e| e.into_inner()).relocate(&shared.cfg.den_dir, spot, &name, area, now());
            shared.den_map.store(map, Ordering::Relaxed);
            shared.event("den_here", &sess.me.name, &format!("map {map} {:.1} {:.1} {:.1}: {name}", p.x, p.y, p.z));
            apply(c, shared, &ch)?;
            shared.autosave();
            Ok(())
        }
        (".den", "grow") | (".den", "size") => {
            let mut d = shared.den.lk();
            let want = if w(1) == "grow" {
                Some(d.ring() + 1)
            } else if w(2).is_empty() {
                None
            } else if w(2) == "city" {
                Some(den::CITY_RING)
            } else {
                match w(2).parse::<u32>() {
                    Ok(n) if (1..=den::RINGS).contains(&n) => Some(n - 1),
                    _ => {
                        drop(d);
                        return say(c, &format!("Sizes are 1 to {} (5 to 7: a whole city): .den size 2, .den size city", den::RINGS));
                    }
                }
            };
            let Some(ring) = want else {
                let l = d.size_line();
                drop(d);
                return say(c, &l);
            };
            let before = d.ring();
            let r = d.resize(ring);
            drop(d);
            match r {
                Ok(ch) if ring > before => {
                    apply(c, shared, &ch)?;
                    jaina_feed(shared, crate::jaina::Event::Grew);
                    cheer(c, shared, sess, "first-grow")
                }
                Ok(ch) => apply(c, shared, &ch),
                Err(e) => say(c, &e),
            }
        }
        // a place copied from the player's own game (stamps.rs copy_places; made by the launcher into stamps-local)
        (".den", "copy") => {
            let places = crate::stamps::copy_places();
            let want = crate::stamps::canonical(w(2)).unwrap_or(w(2));
            match places.iter().find(|pl| pl.0 == want) {
                // a place the importer made on this PC (tools/den-places/den_places.py: `<dens>/stamps-local`)
                None if !want.is_empty() && den::read_stamp(&shared.cfg.stamps_dir, want).is_ok() => jaina_build(c, shared, sess, p, want),
                None => say(c, &format!(
                    "Copy a place from your own game: {}. Places you imported from your own game files (tools/den-places): .den copy <name>, or .den load <name> for a big one.",
                    places.iter().map(|pl| format!(".den copy {} ({})", pl.0, pl.1)).collect::<Vec<_>>().join(", ")
                )),
                Some(&(name, title)) => match den::read_stamp(&shared.cfg.stamps_dir, name) {
                    Ok(_) => jaina_build(c, shared, sess, p, name),
                    Err(_) => say(c, &crate::stamps::not_copied(title)),
                },
            }
        }
        // the Dojo (dojo.rs): the training ring, dummies, flying, speed
        (".den", "dojo") => dojo_cmd(c, shared, sess, p, &words[2..]),
        (".den", "class") => sandbox::class_cmd(c, shared, sess, w(2), &words.iter().skip(3).cloned().collect::<Vec<_>>().join(" ")),
        (".den", "spec") | (".den", "bars") | (".den", "gearset") => dojo_cmd(c, shared, sess, p, &words[1..]),
        (".den", "gear") | (".den", "looks") => sandbox::gear_cmd(c, shared, sess, &words[2..]),
        (".den", "mounts") | (".den", "mount") => sandbox::mounts_cmd(c, shared, sess, w(2), w(3)),
        (".den", "fly") => fly(c, shared, sess, w(2)),
        (".den", "speed") => speed(c, shared, sess, w(2)),
        (".den", "companion") | (".den", "jaina") => match w(2) {
            "build" | "make" => match dojo::build_words(&words[2..].join(" ")) {
                Some(name) => build_or_offer(c, shared, sess, p, &name),
                None => say(c, "Build what? .den jaina build garden (.den stamps lists them; dojo is the training ring)."),
            },
            "away" | "dismiss" | "bye" | "go" => {
                if !shared.companion.lk().out {
                    return say(c, "Your companion isn't here.");
                }
                order(c, shared, p, companion::Cmd::Away)
            }
            "stay" | "wait" => order(c, shared, p, companion::Cmd::Stay),
            "follow" => order(c, shared, p, companion::Cmd::Follow),
            "come" => order(c, shared, p, companion::Cmd::Come),
            "tips" | "tip" | "help" => order(c, shared, p, companion::Cmd::Help),
            // her newer words (jaina.rs): quiet, talk, idea, yes, no, tidy, a pet, pets away, memory (the Den panel's
            // Jaina row sends these)
            _ if crate::jaina::word(&words[2..].join(" ")).is_some() => {
                jaina_word(c, shared, sess, p, crate::jaina::word(&words[2..].join(" ")).unwrap_or(crate::jaina::Word::Memory))
            }
            _ => {
                if !shared.den.lk().inside(p.x, p.y, p.z) {
                    return say(c, "Your companion stays in your Den: come back inside first.");
                }
                let mut comp = shared.companion.lk();
                let was = comp.out;
                let at = comp.come(p, now_ms());
                drop(comp);
                if was {
                    c.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(companion::GUID), target_died: false })?;
                }
                c.send(&companion_create(at))?;
                if was {
                    return monster_say(c, companion::HERE);
                }
                monster_say(c, companion::HELLO)?;
                beat_now(shared);
                say(c, "Your companion follows you. What you tell her goes to your own assistant through the Leyline launcher (Agents page); .den companion away sends her off.")
            }
        },
        (".den", "ask") => {
            let rest = text.trim_start().splitn(3, char::is_whitespace).nth(2).unwrap_or("");
            ask(c, shared, p, rest)
        }
        (".den", "mode") | (".den", "modes") => {
            let lines = if w(2).is_empty() { play::list_lines() } else { play::explain(w(2)) };
            for l in lines {
                say(c, &l)?;
            }
            Ok(())
        }
        (".den", "mods") | (".den", "addons") => {
            for l in play::mods_lines(&sess.addons, shared.cfg.client.as_deref()) {
                say(c, &l)?;
            }
            Ok(())
        }
        (".den", "friends") => friends(c, shared, w(2), w(3), w(4)),
        // duel tournaments (tourney.rs): the host runs one, everyone signs up, yields and watches
        (".den", "tourney") | (".den", "tournament") | (".den", "t") | (".tourney", _) => {
            let from = if w(0) == ".tourney" { 1 } else { 2 };
            for l in crate::tourney::cmd(shared, &sess.me, p.x, p.y, &words[from.min(words.len())..]) {
                say(c, &l)?;
            }
            Ok(())
        }
        (".den", "invite") | (".den", "forge") => ask_launcher(c, shared, w(1)),
        (".den", _) | (".build", _) => say(c, "Den Lite knows: .den place, .den find, .den stamps, .den stamp, .build undo, .den save, .den load, .den files, .den room, .den grow, .den size, .den companion, .den ask, .den mode, .den mods, .den friends, .den invite, .den forge, .den dojo, .den fly, .den speed, .den class, .den gear, .den mounts, .den go, .den here, .den places, .den tourney."),
        // Another server's commands (GM addons send ".server info", ".account onlinelist" on login): not said
        // aloud, and nothing to answer in a Den.
        (w0, _) if w0.starts_with('.') => Ok(()),
        _ => {
            say_aloud(c, shared, sess, text)?;
            // "Jaina, ..." near her: she hears it too (said aloud as usual). The short commands ("Jaina, come") are
            // hers wherever she is in the Den; the rest is an ask, heard within HEAR yards.
            if let Some(words) = companion::heard(text) {
                // "Jaina, build a garden": she builds a stamp (dojo.rs), wherever she is in the Den
                if let Some(name) = dojo::build_words(&words) {
                    return build_or_offer(c, shared, sess, p, &name);
                }
                // her newer words (jaina.rs): quiet, help, yes, no, tidy up, a pet...; then the short commands
                if let Some(wd) = crate::jaina::word(&words) {
                    return jaina_word(c, shared, sess, p, wd);
                }
                if let Some(cmd) = companion::command(&words) {
                    return order(c, shared, p, cmd);
                }
                let comp = shared.companion.lk();
                let near = comp.out && {
                    let at = comp.pos(now_ms());
                    ((at.x - p.x).powi(2) + (at.y - p.y).powi(2)).sqrt() <= companion::HEAR
                };
                drop(comp);
                if near {
                    return ask(c, shared, p, &words);
                }
            }
            Ok(())
        }
    }
}

/// A line said aloud: everyone in the Den hears it from the speaker (their own client first).
fn say_aloud(c: &Writer, shared: &Shared, sess: &Sess, text: &str) -> io::Result<()> {
    let m = SMSG_MESSAGECHAT {
        chat_type: SMSG_MESSAGECHAT_ChatType::Say { target6: Guid::new(sess.me.guid) },
        language: Language::Universal,
        sender: Guid::new(sess.me.guid),
        flags: 0,
        message: text.chars().filter(|c| !c.is_control()).take(255).collect(),
        tag: PlayerChatTag::None,
    };
    c.send(&m)?;
    for o in others(shared, sess.id) {
        let _ = o.w.send(&m);
    }
    Ok(())
}

/// `.den friends` (the host): who is here, and whether friends may build (`build on|off`; one friend: `build <name>
/// on|off`), `kick <name>` (hosting.rs: their login taken back, their game out of the Den). The logins themselves are
/// made by the launcher (`den-lite guest`).
fn friends(c: &Writer, shared: &Shared, a: &str, b: &str, v: &str) -> io::Result<()> {
    if a == "kick" && !b.is_empty() {
        return say(c, &crate::hosting::apply(shared, &crate::hosting::Ask::Kick(b.to_string())));
    }
    if a == "build" && !b.is_empty() && b != "on" && b != "off" && (v == "on" || v == "off") {
        return say(c, &crate::hosting::apply(shared, &crate::hosting::Ask::Build(b.to_string(), v == "on")));
    }
    if a == "build" && (b == "on" || b == "off") {
        shared.guests_build.store(b == "on", Ordering::Relaxed);
        let line = if b == "on" { "Your friends can build in your Den now." } else { "Only you build in your Den now." };
        for p in shared.players.lk().clone() {
            let _ = say(&p.w, line);
        }
        return Ok(());
    }
    let here: Vec<String> = shared.players.lk().iter().filter(|p| !p.me.host).map(|p| p.me.name.clone()).collect();
    let live = crate::guests::list(&shared.cfg.den_dir).into_iter().filter(|g| g.live(now())).count();
    say(c, &format!(
        "Friends here: {}. Invites that still work: {live}. They {} build (.den friends build on|off). Invite friends from the launcher's Your Den page.",
        if here.is_empty() { "none".to_string() } else { here.join(", ") },
        if shared.guests_build.load(Ordering::Relaxed) { "can" } else { "can't" }
    ))
}

/// What a guest types: building only when the host allows it, never the host's own tools (saves, files, the
/// companion, the assistant); everything else is said aloud.
fn guest_chat(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, text: &str) -> io::Result<()> {
    let words: Vec<String> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
    let w = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    if !text.trim_start().starts_with('.') {
        return say_aloud(c, shared, sess, text);
    }
    if w(0) == ".tourney" {
        return chat(c, shared, sess, p, text);
    }
    if !matches!(w(0), ".den" | ".build") {
        return Ok(());
    }
    let builds = matches!((w(0), w(1)), (".den", "place") | (".den", "stamp") | (".build", "undo"));
    let looks = matches!((w(0), w(1)), (".den", "find") | (".den", "stamps") | (".den", "mode") | (".den", "modes") | (".den", "mods") | (".den", "addons"))
        || (w(0) == ".den" && w(1) == "size" && w(2).is_empty());
    // tournaments: friends sign up, yield and read the bracket (tourney.rs says no to the host's verbs)
    if matches!((w(0), w(1)), (".den", "tourney") | (".den", "tournament") | (".den", "t")) {
        return chat(c, shared, sess, p, text);
    }
    if looks || (builds && crate::hosting::may_build(shared, &sess.me)) {
        return chat(c, shared, sess, p, text);
    }
    if builds {
        return say(c, "Your host builds here. They can let friends build too: .den friends build on.");
    }
    say(c, "As a guest you can: .den find, .den stamps, .den size, .den mode, .den mods, .den tourney (and build, when your host lets you).")
}

/// An ask for the player's assistant: written down (companion-in.json `asks`), and she answers.
fn ask(c: &Writer, shared: &Shared, p: Pos, text: &str) -> io::Result<()> {
    let (spot, inside) = {
        let d = shared.den.lk();
        (d.spot, d.inside(p.x, p.y, p.z))
    };
    let rel = inside.then_some(At { x: p.x - spot.x, y: p.y - spot.y, z: p.z - spot.z, o: p.o });
    let r = shared.companion.lk().ask(&shared.cfg.den_dir, text, rel, now());
    match r {
        Err(e) => say(c, &e),
        Ok(Asked::Busy(line)) => monster_say(c, line),
        Ok(Asked::Noted(_, line)) => {
            beat_now(shared);
            monster_say(c, line)
        }
    }
}

/// The companion's create: a unit (create type 2, flags living), her entry and model, friendly to all, immune to
/// players and creatures (unit_flags 832, as her creature_template), no npc flags (no gossip, vendor or quests).
fn companion_create(at: At) -> SMSG_UPDATE_OBJECT {
    let guid = Guid::new(companion::GUID);
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(guid)
        .set_object_entry(companion::ENTRY as i32)
        .set_object_scale_x(1.0)
        .set_unit_bytes_0(Race::Human, Class::Mage, Gender::Female, Power::Mana)
        .set_unit_health(100)
        .set_unit_maxhealth(100)
        .set_unit_level(80)
        .set_unit_factiontemplate(35)
        .set_unit_flags(832)
        .set_unit_boundingradius(0.389)
        .set_unit_combatreach(1.5)
        .set_unit_displayid(companion::DISPLAY as i32)
        .set_unit_nativedisplayid(companion::DISPLAY as i32)
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
        running_speed: companion::RUN,
        swimming_speed: 4.722222,
        timestamp: 0,
        turn_rate: std::f32::consts::PI,
        walking_speed: companion::WALK,
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

/// CMSG_CREATURE_QUERY: her name (what the client shows over her), "<Your companion>" under it, humanoid, her model.
fn creature_answer(entry: u32) -> SMSG_CREATURE_QUERY_RESPONSE {
    if let Some(a) = crate::farsight::creature(entry) {
        return a;
    }
    if let Some(pet) = sandbox::pet_answer(entry) {
        return pet;
    }
    if let Some((_, display, name, _)) = dojo::dummy(entry) {
        return SMSG_CREATURE_QUERY_RESPONSE {
            creature_entry: entry,
            found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
                name1: name.into(),
                name2: String::new(),
                name3: String::new(),
                name4: String::new(),
                sub_name: String::new(),
                description: String::new(),
                type_flags: 0,
                creature_type: 9, // mechanical
                creature_family: CreatureFamily::None,
                creature_rank: sandbox::rank_of(entry),
                kill_credit1: 0,
                kill_credit2: 0,
                display_ids: [display, 0, 0, 0],
                health_multiplier: 1.0,
                mana_multiplier: 1.0,
                racial_leader: 0,
                quest_items: [0; 6],
                movement_id: 0,
            }),
        };
    }
    if let Some(a) = jaina_world::critter_answer(entry) {
        return a;
    }
    if entry != companion::ENTRY {
        return SMSG_CREATURE_QUERY_RESPONSE { creature_entry: entry | 0x8000_0000, found: None };
    }
    SMSG_CREATURE_QUERY_RESPONSE {
        creature_entry: entry,
        found: Some(SMSG_CREATURE_QUERY_RESPONSE_found {
            name1: companion::NAME.into(),
            name2: String::new(),
            name3: String::new(),
            name4: String::new(),
            sub_name: companion::TITLE.into(),
            description: String::new(),
            type_flags: 0,
            creature_type: 7, // humanoid
            creature_family: CreatureFamily::None,
            creature_rank: 0,
            kill_credit1: 0,
            kill_credit2: 0,
            display_ids: [companion::DISPLAY, 0, 0, 0],
            health_multiplier: 1.0,
            mana_multiplier: 1.0,
            racial_leader: 0,
            quest_items: [0; 6],
            movement_id: 0,
        }),
    }
}

/// A line in her voice (monster say: "Jaina says: ...").
fn monster_say(c: &Writer, text: &str) -> io::Result<()> {
    c.send(&SMSG_MESSAGECHAT {
        chat_type: SMSG_MESSAGECHAT_ChatType::MonsterSay { sender1: companion::NAME.into(), target1: NamedGuid::zero() },
        language: Language::Universal,
        sender: Guid::new(companion::GUID),
        flags: 0,
        message: text.into(),
        tag: PlayerChatTag::None,
    })
}

/// Her walk from one spot to another (SMSG_MONSTER_MOVE, one point, no spline flags: the client runs or walks her
/// there in `ms`); around a prop (`via`, flight.rs), a curved path through its waypoints (Catmullrom: every point
/// written whole).
fn walk(c: &Writer, from: At, to: At, ms: u32, id: u32, via: &[At]) -> io::Result<()> {
    if !via.is_empty() {
        let mut pts = via.to_vec();
        pts.push(to);
        return c.send_raw(crate::flight::SMSG_MONSTER_MOVE, &crate::flight::sky_body(companion::GUID, from, &pts, id, crate::flight::SPLINE_CATMULLROM, ms));
    }
    c.send(&SMSG_MONSTER_MOVE {
        guid: Guid::new(companion::GUID),
        unknown: 0,
        spline_point: Vector3d { x: from.x, y: from.y, z: from.z },
        spline_id: id,
        move_type: SMSG_MONSTER_MOVE_MonsterMoveType::Normal,
        spline_flags: SMSG_MONSTER_MOVE_SplineFlag::empty(),
        duration: ms,
        splines: vec![Vector3d { x: to.x, y: to.y, z: to.z }],
    })
}

/// The heartbeat's `owner` and `room` (companion-in.json, as ov_den_companion.lua writes them).
fn beat_parts(shared: &Shared) -> (serde_json::Value, serde_json::Value) {
    use serde_json::json;
    let live = shared.live.lk().as_ref().map(|l| l.2);
    let d = shared.den.lk();
    let r1 = |v: f32| (v as f64 * 10.0).round() / 10.0;
    let owner = match live {
        Some(p) => json!({"online": true, "inside": d.inside(p.x, p.y, p.z), "x": r1(p.x - d.spot.x), "y": r1(p.y - d.spot.y),
            "z": r1(p.z - d.spot.z), "yaw": r1(p.o.to_degrees().rem_euclid(360.0))}),
        None => json!({"online": false}),
    };
    let room = json!({"props": d.props.len(), "half": d.spot.half, "r": d.spot.reach(), "size": d.ring() + 1, "sizes": den::RINGS,
        "look": d.look.name});
    (owner, room)
}

fn beat_now(shared: &Shared) {
    let (owner, room) = beat_parts(shared);
    shared.companion.lk().beat(&shared.cfg.den_dir, now(), true, owner, room);
}

/// Twice a second, for as long as Den Lite runs: the companion follows the player in the world and says the lines
/// the assistant wrote (companion-out.json), and the heartbeat (companion-in.json) is written every 10 seconds.
pub fn companion_ticker(shared: Arc<Shared>) {
    let mut n: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        n += 1;
        let _ = companion_tick(&shared, n % 2 == 0);
        notices_due(&shared);
        // every 2 seconds, when the Den changed; the Den bar shows "Saved 2 s ago"
        if n % 4 == 0 && shared.autosave() {
            let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| (l.0, l.1.clone()));
            if let Some((sess, w)) = live {
                let _ = bar_data(&w, &shared, sess, PLAYER_GUID, "saved", "auto");
            }
        }
    }
}

/// The props' footprints she walks around (flight.rs), from their models' sizes when the table is there.
fn companion_blocks(d: &den::Den) -> Vec<crate::flight::Block> {
    d.props.iter().map(|x| crate::flight::block_of(x.obj.x, x.obj.y, x.obj.scale, crate::models::tile(x.obj.display).as_deref())).collect()
}

/// Every 300 ms or so (dojo_ticker): she flies with a flying player and lands with them (flight.rs).
fn flight_tick(shared: &Shared) -> io::Result<()> {
    use crate::flight;
    // one clock per Den Lite (each runs its own dojo_ticker thread; a shared one starved the others in tests)
    thread_local!(static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) });
    let ms = now_ms();
    if ms < NEXT.with(|n| n.get()) {
        return Ok(());
    }
    NEXT.with(|n| n.set(ms + flight::SKY_EVERY_MS));
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some((_, w, p)) = live else { return Ok(()) };
    let (fly_on, speed) = { let d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()); (d.fly, d.speed_now()) };
    let (ground, land) = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let s = companion::spot(p);
        (d.floor_at(p.x, p.y), d.floor_at(s.x, s.y))
    };
    let up = flight::airborne(fly_on, p, ground);
    let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
    if !comp.out || comp.order == companion::Order::Stay {
        return Ok(());
    }
    let here = comp.pos(ms);
    if up {
        let first = !comp.flying;
        let target = flight::sky_spot(p);
        if !first && (comp.pos(u64::MAX).x - target.x).hypot(comp.pos(u64::MAX).y - target.y) <= flight::SKY_SLACK && (comp.pos(u64::MAX).z - target.z).abs() <= flight::SKY_SLACK {
            return Ok(());
        }
        comp.flying = true;
        let (pts, t) = flight::sky_path(here, target, dojo::BASE_FLIGHT * speed as f32);
        let id = comp.set_path(here, target, ms, t);
        drop(comp);
        if first {
            w.send_raw(flight::SMSG_SPLINE_MOVE_SET_FLYING, &flight::guid_body(companion::GUID))?;
            w.send_raw(flight::SMSG_SPLINE_MOVE_GRAVITY_DISABLE, &flight::guid_body(companion::GUID))?;
            shared.event("companion", "", "flies");
            jaina_feed(shared, crate::jaina::Event::FlyStart);
        }
        w.send_raw(flight::SMSG_MONSTER_MOVE, &flight::sky_body(companion::GUID, here, &pts, id, flight::SPLINE_FLYING | flight::SPLINE_CATMULLROM, t))?;
    } else if comp.flying {
        // you landed: she lands beside you
        comp.flying = false;
        let (pts, t) = flight::landing(here, p, land);
        let to = pts[pts.len() - 1];
        let id = comp.set_path(here, to, ms, t);
        drop(comp);
        w.send_raw(flight::SMSG_MONSTER_MOVE, &flight::sky_body(companion::GUID, here, &pts, id, flight::SPLINE_FLYING, t))?;
        w.send_raw(flight::SMSG_SPLINE_MOVE_UNSET_FLYING, &flight::guid_body(companion::GUID))?;
        w.send_raw(flight::SMSG_SPLINE_MOVE_GRAVITY_ENABLE, &flight::guid_body(companion::GUID))?;
        shared.event("companion", "", "lands");
        jaina_feed(shared, crate::jaina::Event::FlyEnd);
    }
    Ok(())
}

fn companion_tick(shared: &Shared, read: bool) -> io::Result<()> {
    let (now, ms) = (now(), now_ms());
    let live = shared.live.lk().clone();
    if read {
        shared.companion.lk().read_lines(&shared.cfg.den_dir, now);
    }
    if let Some((_, w, p)) = live {
        // she keeps up anywhere near the Den (flight.rs AREA: past the Den's own circle she used to go, "fading
        // away"); the props she walks around
        // only on the Den's map (`.den go` elsewhere: she waits in the Den)
        let on_map = map_of(shared, &w).map_or(true, |m| m == den_map(shared));
        let (inside, near, blocks) = {
            let d = shared.den.lk();
            (on_map && d.inside(p.x, p.y, p.z), on_map && crate::flight::near_den(d.spot.x, d.spot.y, p), companion_blocks(&d))
        };
        let (out, wanted) = { let c = shared.companion.lk(); (c.out, c.wanted) };
        if out {
            let mut comp = shared.companion.lk();
            comp.blocks = blocks;
            if !inside && !near {
                let left = comp.brain.leftovers();
                comp.away();
                drop(comp);
                w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(companion::GUID), target_died: false })?;
                for g in left {
                    w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false })?;
                }
                notify_host(shared, Item::line("info", "Your companion stays in your Den and joins you again when you're back."))?;
                beat_now(shared);
            } else if comp.too_far(p, ms) {
                let at = comp.come(p, ms);
                drop(comp);
                w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(companion::GUID), target_died: false })?;
                w.send(&companion_create(at))?;
            } else {
                // a hop that came down: she walks again
                if comp.landed(ms) {
                    w.send_raw(crate::flight::SMSG_SPLINE_MOVE_UNSET_FLYING, &crate::flight::guid_body(companion::GUID))?;
                    w.send_raw(crate::flight::SMSG_SPLINE_MOVE_GRAVITY_ENABLE, &crate::flight::guid_body(companion::GUID))?;
                }
                let step = comp.follow(p, ms);
                let hop = comp.take_hop();
                let (id, via) = (comp.moves, comp.via.clone());
                // only the assistant's answers (she was asked): no tips nobody asked for (owner, 1 Nov 23:46)
                let line = comp.next_line(now);
                drop(comp);
                if let Some((from, to, t)) = step {
                    step_send(&w, from, to, t, id, &via, hop)?;
                }
                if let Some(l) = line {
                    monster_say(&w, &l)?;
                }
                // what she does by herself (jaina.rs): gestures, a look around, now and then a word or an idea
                brain_tick(shared, &w, p, ms)?;
            }
        } else if wanted && inside {
            // back in the Den: she joins you again by herself, quietly
            let at = shared.companion.lk().come(p, ms);
            w.send(&companion_create(at))?;
            beat_now(shared);
        }
    }
    let (owner, room) = beat_parts(shared);
    shared.companion.lk().beat(&shared.cfg.den_dir, now, false, owner, room);
    Ok(())
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
                // the launcher asked: its lines are toasts in the Den bar (notice.rs), when the host has it
                let sess = shared.live.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|l| l.0).unwrap_or(0);
                if shared.mark_opened(&dir, &id) && quietly(&w, &shared, sess, PLAYER_GUID, || check_open(&shared, &w, &file)).is_err() {
                    break;
                }
            }
            // the launcher's showcase (showcase.rs, a share from Mods & Addons › Dens): the camera over the build, and
            // the Den bar takes the pictures with the game's own Screenshot()
            if let Some(id) = crate::showcase::take_ask(&dir) {
                if run_showcase(&shared, &w, &dir, &id).is_err() {
                    break;
                }
            }
            // the launcher's ground file (after Forge closes): props and stamps stand on the new ground at once;
            // the game itself shows it after a restart (its map files load at start)
            let changed = shared.den.lk().reload_ground(&dir);
            let line = match changed {
                Some(Ok(true)) => Some(Item::line("ok", "Your Den's ground changed: new props stand on it now. Restart your game to see it.")),
                Some(Err(_)) => Some(Item::line("warn", "Your Den's ground file couldn't be read: props stand on the flat floor.")),
                _ => None,
            };
            if let Some(l) = line {
                if notify_host(&shared, l).is_err() {
                    break;
                }
            }
            // the launcher's answer to the game's ask (ovmod.rs): the invite link into the Den panel, or a line
            let sent = match ovmod::take_answer(&dir) {
                Some(ovmod::Answer::Invite(link)) => {
                    // never the link itself in the log: it holds the Den's secret and the friend's password
                    shared.event("answer", "", "invite");
                    let me = PLAYER_GUID;
                    let mut ok = true;
                    for b in ovmod::answer_bodies(me, "wuden", "invite", &link) {
                        ok &= w.send_raw(ovmod::SMSG_MESSAGECHAT, &b).is_ok();
                    }
                    ok &= notify_host(&shared, Item {
                        kind: "ok",
                        toast: Some("Your invite link is ready: Ctrl+C copies it. Give it to one friend.".into()),
                        old: vec![Old::Line("Your invite link is in the Den panel (/den): Ctrl+C copies it. Give it to one friend.".into())],
                    }).is_ok();
                    ok
                }
                Some(ovmod::Answer::Say(t)) => {
                    shared.event("answer", "", "say");
                    notify_host(&shared, Item::line("info", t)).is_ok()
                }
                None => true,
            };
            if !sent {
                break;
            }
            for _ in 0..20 {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                if shared.cfg.test_control {
                    if let Some((id, cmd)) = testctl::take(&dir) {
                        if test_command(&shared, &w, id, &cmd).is_err() {
                            return;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    StopOnDrop(stop)
}

/// One of the den-test's commands (testctl.rs), run as the host typing it where they stand, and what it said and
/// did written to `den-lite.test-out`.
fn test_command(shared: &Shared, w: &Writer, id: u64, cmd: &str) -> io::Result<()> {
    let Some((sess_id, _, p)) = shared.live.lock().unwrap_or_else(|e| e.into_inner()).clone() else { return Ok(()) };
    let sess = Sess { id: sess_id, addons: Vec::new(), me: Me::host(shared) };
    shared.event("test_cmd", "", &cmd.split_whitespace().take(2).collect::<Vec<_>>().join(" "));
    let (r, cap) = testctl::capture(|| chat(w, shared, &sess, p, cmd));
    let (props, look, size, ground) = { let d = shared.den.lock().unwrap_or_else(|e| e.into_inner()); (d.props.len(), d.look.name, d.ring() + 1, d.floor_at(p.x, p.y)) };
    // where the host stands and what their game did with its character (movewatch.rs), for the den-test's walk check
    let me = shared.moves.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|w| w.sess == sess_id).map(|w| serde_json::json!({
        "mover": w.mover, "moves": w.moves, "first": w.first.map(movewatch::op_name), "walked": w.walked,
        "landed": w.landed.map(|(z, g)| z - g), "flags": movewatch::flag_names(w.flags_seen & !movewatch::FALLING),
        "spawn": [w.spawn.x, w.spawn.y, w.spawn.z], "spawn_ground": w.ground,
        "ops": w.ops_line(), "time_sync": w.time_sync.is_some(),
    }));
    let prop_flags = {
        let build = shared.build_look.load(Ordering::Relaxed);
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let f: Option<u32> = d.props.first().map(|p| gizmo::shown(&p.obj, build).1);
        f
    };
    testctl::write_out(&shared.cfg.den_dir, &serde_json::json!({
        "id": id, "cmd": cmd, "said": cap.said, "created": cap.created, "destroyed": cap.destroyed,
        "props": props, "look": look, "size": size, "t": now_ms(), "ok": r.is_ok(),
        "pos": [p.x, p.y, p.z], "ground": ground, "on_ground": (p.z - ground).abs() <= movewatch::ON_GROUND, "me": me,
        "aids": shared.aid_view.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        "look_mode": if shared.build_look.load(Ordering::Relaxed) { "build" } else { "play" },
        "twin_queries": shared.twin_queries.load(Ordering::Relaxed),
        // the flags the game is told for a prop now (gizmo::shown): NOT_SELECTABLE in build mode (5 Nov, den-move)
        "prop_flags": prop_flags,
    }));
    r
}

/// The showcase (showcase.rs): the far camera's object over the middle of what is built and the view on it, the Den
/// bar told to take the pictures, then the view given back and the answer written for the launcher. Not in the world
/// (no session): the answer says no pictures.
fn run_showcase(shared: &Shared, w: &Writer, dir: &std::path::Path, id: &str) -> io::Result<()> {
    use crate::{farsight, flight, showcase};
    let (points, home) = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        (d.props.iter().map(|p| (p.obj.x, p.obj.y, p.obj.z)).collect::<Vec<_>>(), (d.spot.x, d.spot.y, d.spot.z))
    };
    let (x, y, z, r) = showcase::pose(&points, home);
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    if !live {
        let _ = std::fs::write(dir.join(showcase::DONE), showcase::done_doc(id, r, false));
        return Ok(());
    }
    shared.event("showcase", "", &format!("start {} props r{}", points.len(), r.round()));
    w.send(&farsight::create(farsight::at([x, y, z, 0.0])))?;
    w.send_raw(flight::SMSG_SPLINE_MOVE_SET_FLYING, &flight::guid_body(farsight::GUID))?;
    w.send_raw(flight::SMSG_SPLINE_MOVE_GRAVITY_DISABLE, &flight::guid_body(farsight::GUID))?;
    w.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::farsight(PLAYER_GUID, farsight::GUID))?;
    send_addon(w, PLAYER_GUID, "wuden", "showcase", &showcase::addon_line(r))?;
    std::thread::sleep(Duration::from_millis(showcase::total_ms() as u64));
    w.send_raw(raw::SMSG_UPDATE_OBJECT, &raw::farsight(PLAYER_GUID, 0))?;
    w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(farsight::GUID), target_died: false })?;
    let _ = std::fs::write(dir.join(showcase::DONE), showcase::done_doc(id, r, true));
    shared.event("showcase", "", "done");
    Ok(())
}

fn check_open(shared: &Shared, w: &Writer, file: &str) -> io::Result<()> {
    let doc = match den::read_file(&shared.cfg.den_dir.join(format!("{file}.den.json"))) {
        Ok(d) => d,
        Err(e) => return say(w, &format!("The launcher asked to open '{file}': {e}")),
    };
    say(w, &format!("Opening '{file}' from the launcher."))?;
    let mut d = shared.den.lk();
    if !d.props.is_empty() {
        let _ = d.save(&shared.cfg.den_dir, "backup", now());
    }
    let ch = d.open(&doc, true);
    drop(d);
    apply(w, shared, &ch)
}

// ---------------------------------------------------------------- the Dojo (dojo.rs): training grounds without combat

/// The Den bar's Dojo panel line (`wuden dojo <line>`), only when the bar is there.
fn dojo_line(c: &Writer, shared: &Shared, sess: &Sess) -> io::Result<()> {
    let line = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).line() + &sandbox::line_extra(shared);
    bar_data(c, shared, sess.id, sess.me.guid, "dojo", &line)?;
    if sess.me.host {
        jaina_panel(shared, true)?;
    }
    Ok(())
}

/// `.den fly on|off` (alone: switches it): the game lets the character fly, or lands it.
fn fly(c: &Writer, shared: &Shared, sess: &Sess, w: &str) -> io::Result<()> {
    let cur = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).fly;
    let on = if w.is_empty() {
        !cur
    } else {
        match dojo::on_off(w) {
            Some(b) => b,
            None => return say(c, "Usage: .den fly on or .den fly off (.den fly alone switches it)."),
        }
    };
    let n = {
        let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
        d.fly = on;
        d.counter()
    };
    let op = if on { dojo::SMSG_MOVE_SET_CAN_FLY } else { dojo::SMSG_MOVE_UNSET_CAN_FLY };
    c.send_raw(op, &dojo::can_fly_body(sess.me.guid, n))?;
    shared.event("dojo", &sess.me.name, if on { "fly on" } else { "fly off" });
    say(c, if on { "You can fly: jump, then hold Space to rise and X to sink. .den fly off lands you." } else { "Flying is off: you come down where you are." })?;
    dojo_line(c, shared, sess)
}

/// `.den speed 1-5`: run and flight speed, times your normal.
fn speed(c: &Writer, shared: &Shared, sess: &Sess, w: &str) -> io::Result<()> {
    if w.is_empty() {
        let n = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).speed_now();
        return say(c, &format!("Your speed: {n} (.den speed 1 to {}).", dojo::MAX_SPEED));
    }
    let n = match dojo::speed_arg(w) {
        Ok(n) => n,
        Err(e) => return say(c, &e),
    };
    let (a, b) = {
        let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
        d.speed = n;
        (d.counter(), d.counter())
    };
    c.send_raw(dojo::SMSG_FORCE_RUN_SPEED_CHANGE, &dojo::run_speed_body(sess.me.guid, a, dojo::BASE_RUN * n as f32))?;
    c.send_raw(dojo::SMSG_FORCE_FLIGHT_SPEED_CHANGE, &dojo::flight_speed_body(sess.me.guid, b, dojo::BASE_FLIGHT * n as f32))?;
    shared.event("dojo", &sess.me.name, &format!("speed {n}"));
    say(c, &if n == 1 { "Normal speed.".to_string() } else { format!("You run and fly {n} times as fast (.den speed 1: normal).") })?;
    dojo_line(c, shared, sess)
}

const DOJO_HELP: &[&str] = &[
    ".den dojo: a training ring beside you, built by your companion, with three training dummies in it (or .dojo)",
    ".den dojo dummies [1-5]: training dummies (in the ring, or in front of you); .den dojo clear: the dummies down",
    ".den fly on|off: fly (jump, then Space to rise, X to sink); .den speed 1-5: run and fly faster",
    ".den jaina build <stamp>, or say \"Jaina, build a garden\": she walks over and builds it piece by piece",
    ".den dojo pack|mover|healer|tank|boss: dummies to practise area spells, chasing, healing, tanking and a boss's circles",
    ".den dojo parse 1|3|5, reset, refill, cooldowns, fight on|off, best, dps; .den class <class> [level]: any class",
    "On the full Den (Leyline > Host > Your Den): duels, a new look.",
];

/// `.den dojo [ring|dummies [n]|clear|fly|speed|build <stamp>|status|help]`.
fn dojo_cmd(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, args: &[String]) -> io::Result<()> {
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    // the sandbox combat's verbs (sandbox.rs): the other dummies, parses, reset, refill, cooldowns, bests
    if let Some(r) = sandbox::dojo_verb(c, shared, sess, p, args) {
        return r.and_then(|_| dojo_line(c, shared, sess));
    }
    match a(0) {
        "" | "ring" | "here" | "on" => jaina_build(c, shared, sess, p, "dojo"),
        "dummies" | "dummy" | "dummys" => {
            let n = a(1).parse::<usize>().unwrap_or(3);
            let made = dummies_up(shared, p, n);
            say(c, &format!("{made} training dummies up: target one and attack, or cast at it. Your damage per second shows (.den dojo help: more dummies)."))?;
            dojo_line(c, shared, sess)
        }
        "clear" | "off" | "down" => {
            let (gone, building) = {
                let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
                (d.clear_dummies(), d.job.take().is_some())
            };
            for pl in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
                for &g in &gone {
                    let _ = pl.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false });
                }
            }
            if building {
                shared.companion.lock().unwrap_or_else(|e| e.into_inner()).follow_on();
            }
            shared.event("dojo", &sess.me.name, &format!("clear {}", gone.len()));
            say(c, &format!("{} dummies down{}. The ring stays: .build undo takes it back.", gone.len(), if building { ", the build stopped" } else { "" }))?;
            dojo_line(c, shared, sess)
        }
        "fly" => fly(c, shared, sess, a(1)),
        "speed" => speed(c, shared, sess, a(1)),
        "build" | "jaina" | "make" => match dojo::build_words(&args.join(" ")).or_else(|| (a(0) == "jaina").then(|| "dojo".into())) {
            Some(name) => build_or_offer(c, shared, sess, p, &name),
            None => say(c, "Build what? .den dojo build garden (.den stamps lists them)."),
        },
        "status" => dojo_line(c, shared, sess),
        "duel" | "duels" | "look" | "barber" | "queue" | "dungeon" | "raid" => {
            say(c, &format!("That {}. Here: .den dojo, .den dojo dummies, .den fly, .den speed, \"Jaina, build ...\".", dojo::FULL_ONLY))?;
            dojo_line(c, shared, sess)
        }
        _ => {
            for l in DOJO_HELP {
                say(c, l)?;
            }
            dojo_line(c, shared, sess)
        }
    }
}

/// `n` training dummies (the old ones down first) for everyone in the world: how many went up.
fn dummies_up(shared: &Shared, p: Pos, n: usize) -> usize {
    // never the Den's lock and the Dojo's together (dojo_tick takes them one after the other too)
    let at = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).ring.unwrap_or(p);
    let z = shared.den.lock().unwrap_or_else(|e| e.into_inner()).floor_at(at.x, at.y);
    let (gone, made) = {
        let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
        (d.clear_dummies(), d.place_dummies(p, n, dojo::DUMMIES[0].0, z))
    };
    for pl in shared.players.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        for &g in &gone {
            let _ = pl.w.send(&SMSG_DESTROY_OBJECT { guid: Guid::new(g), target_died: false });
        }
        for m in &made {
            let _ = pl.w.send(&dummy_create(shared, m));
        }
    }
    shared.event("dojo", "", &format!("dummies {}", made.len()));
    made.len()
}

/// `.den jaina build <stamp>` / "Jaina, build a garden" / `.den dojo` (the stamp "dojo": the training ring, then its
/// dummies): the stamp goes into the Den at once (saved, one undo) in front of the player; with the companion out
/// she walks over and the game is shown it piece by piece (dojo_ticker), else all at once.
fn jaina_build(c: &Writer, shared: &Shared, sess: &Sess, p: Pos, name: &str) -> io::Result<()> {
    let ring = name == "dojo" || name == "ring" || name == "training";
    let st = if ring { den::parse(dojo::DOJO_STAMP) } else { den::read_stamp(&shared.cfg.stamps_dir, name) };
    let st = match st {
        Ok(st) => st,
        Err(e) => return say(c, &format!("{e} .den stamps lists them; dojo is the training ring.")),
    };
    let out = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).out;
    if shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job.is_some() {
        return if out { monster_say(c, dojo::BUSY) } else { say(c, dojo::BUSY) };
    }
    // in front of the player (snapped to the stamps' 15 degree steps), its middle far enough that it all fits there
    let step = std::f32::consts::PI / 12.0;
    let o = ((p.o / step).round() * step).rem_euclid(2.0 * std::f32::consts::PI);
    let me = At { o, ..p };
    let far = dojo::reach(&st.props.iter().map(|e| (e.x, e.y)).collect::<Vec<_>>());
    // then moved toward the room's middle as far as it takes for all of it to be inside the Den (den.rs fit_stamp)
    let mid = dojo::ahead(me, far);
    let (fx0, fy0, too_big) = shared.den.lock().unwrap_or_else(|e| e.into_inner()).fit_stamp(&st, mid.x, mid.y, o);
    let mid = At { x: fx0, y: fy0, ..mid };
    // on the ground there (a player flying overhead builds on the ground, not in the air)
    let origin = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        At { z: d.floor_at(mid.x, mid.y), ..mid }
    };
    let ch = shared.den.lock().unwrap_or_else(|e| e.into_inner()).stamp(&st, origin.x, origin.y, origin.z, o);
    if ch.created.is_empty() {
        return say(c, &ch.message);
    }
    if too_big {
        say(c, "It is bigger than your Den: some pieces are left out. .den grow makes the room bigger.")?;
    }
    // she builds from its middle (the free spot nearest its focal point: never inside a piece) and, when it is done,
    // steps back to your side to look at it (stamps.rs)
    let (fx, fy) = crate::stamps::focus(&st);
    let (centre, stand, admire) = {
        let d = shared.den.lock().unwrap_or_else(|e| e.into_inner());
        let f = dojo::local(origin, fx as f32, fy as f32);
        let f = At { z: d.floor_at(f.x, f.y), ..f };
        let s = dojo::build_stand(f, &ch.created);
        let a = dojo::admire_spot(f, &ch.created, me);
        (f, At { z: d.floor_at(s.x, s.y), ..s }, At { z: d.floor_at(a.x, a.y), ..a })
    };
    if ring {
        shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).ring = Some(origin);
    }
    let title = st.title.clone().or(st.name.clone()).unwrap_or_else(|| name.to_string());
    shared.event("dojo", &sess.me.name, &format!("build {name} {}", ch.created.len()));
    if !out {
        // no companion out: all of it at once, as a stamp
        apply(c, shared, &ch)?;
        if ring {
            dummies_up(shared, p, 3);
        }
        say(c, "(Call your companion, .den companion, and she builds it for you piece by piece.)")?;
        return dojo_line(c, shared, sess);
    }
    shared.event("den_change", "", &format!("+{} -0", ch.created.len()));
    testctl::note_change(ch.created.len(), 0);
    // friends see it at once; the host watches her build it
    for o in others(shared, sess.id) {
        let _ = send_creates(&o.w, shared, &ch.created);
    }
    let ms = now_ms();
    let walked = {
        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
        comp.heard_now(now());
        // what she builds is hers until you move it ("Jaina, tidy up" may take it away, after asking)
        comp.brain.adopt(&ch.created.iter().map(|o| (o.guid, o.x, o.y, o.z)).collect::<Vec<_>>());
        comp.go_stay(stand, ms).map(|w| (w, comp.moves, comp.via.clone()))
    };
    let walk_ms = match walked {
        Some(((from, to, d), id, via)) => {
            walk(c, from, to, d, id, &via)?;
            d as u64
        }
        None => 0,
    };
    monster_say(c, dojo::ON_IT)?;
    let mut job = dojo::Job::new(stand, centre, ch.created, ms, walk_ms, &title);
    job.then_dummies = ring;
    job.admire = Some(admire);
    shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job = Some(job);
    dojo_line(c, shared, sess)
}

/// A dummy's create: a unit like the companion's (dojo::DUMMIES / KIND_DUMMIES: its entry, model and level),
/// standing still, with the sandbox combat's health and faction (sandbox.rs: attackable, or friendly for the wounded
/// one), no flags that keep you from hitting it.
fn dummy_create(shared: &Shared, d: &dojo::Dummy) -> SMSG_UPDATE_OBJECT {
    let (entry, display, _, level) = dojo::dummy(d.entry).unwrap_or(dojo::DUMMIES[0]);
    let (cur, max, faction, scale) = sandbox::dummy_look(shared, d);
    let guid = Guid::new(d.guid);
    let mask = UpdateUnitBuilder::new()
        .set_object_guid(guid)
        .set_object_entry(entry as i32)
        .set_object_scale_x(scale)
        .set_unit_bytes_0(Race::Human, Class::Warrior, Gender::Male, Power::Rage)
        .set_unit_health(cur as i32)
        .set_unit_maxhealth(max as i32)
        .set_unit_level(level.into())
        .set_unit_factiontemplate(faction as i32)
        .set_unit_flags(0)
        .set_unit_boundingradius(0.389)
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
        backwards_swimming_speed: 2.5,
        fall_time: 0.0,
        flags: MovementBlock_MovementFlags::empty(),
        flight_speed: 7.0,
        orientation: d.at.o,
        pitch_rate: std::f32::consts::PI,
        position: Vector3d { x: d.at.x, y: d.at.y, z: d.at.z },
        running_speed: 7.0,
        swimming_speed: 4.722222,
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

/// Every 50 ms for as long as Den Lite runs: the companion's build in progress (dojo::Job), shown to the host.
pub fn dojo_ticker(shared: Arc<Shared>) {
    loop {
        std::thread::sleep(Duration::from_millis(50));
        let _ = dojo_tick(&shared);
    }
}

fn dojo_tick(shared: &Shared) -> io::Result<()> {
    sandbox::tick(shared);
    let _ = flight_tick(shared);
    if shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job.is_none() {
        return Ok(());
    }
    let live = shared.live.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some((sess_id, w, p)) = live else {
        shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).job = None;
        return Ok(());
    };
    let ms = now_ms();
    // the props still in the Den (an undo while she builds takes the rest back), before the Dojo's lock
    let placed: std::collections::HashSet<u64> = shared.den.lock().unwrap_or_else(|e| e.into_inner()).props.iter().map(|x| x.obj.guid).collect();
    let (steps, done, ring_job) = {
        let mut d = shared.dojo.lock().unwrap_or_else(|e| e.into_inner());
        let Some(job) = d.job.as_mut() else { return Ok(()) };
        let steps = job.due(ms, |g| placed.contains(&g));
        let ring_job = job.then_dummies;
        let done = steps.iter().any(|s| matches!(s, dojo::Step::Done { .. })).then(|| (job.title.clone(), job.then_dummies));
        if done.is_some() {
            d.job = None;
        }
        (steps, done, ring_job)
    };
    let out = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).out;
    for s in steps {
        match s {
            dojo::Step::Start { ms: cast, at } => {
                if out {
                    w.send_raw(dojo::SMSG_SPELL_START, &dojo::spell_start_body(companion::GUID, dojo::BUILD_SPELL, cast, at))?;
                }
            }
            dojo::Step::Piece { obj, facing } => {
                if out {
                    let (here, id) = shared.companion.lock().unwrap_or_else(|e| e.into_inner()).face(facing, ms);
                    w.send_raw(dojo::SMSG_MONSTER_MOVE, &dojo::facing_body(companion::GUID, here, id, facing))?;
                    w.send_raw(dojo::SMSG_SPELL_GO, &dojo::spell_go_body(companion::GUID, dojo::BUILD_SPELL, ms as u32, [obj.x, obj.y, obj.z]))?;
                }
                send_creates(&w, shared, std::slice::from_ref(&obj))?;
            }
            dojo::Step::Admire { to } => {
                // the last piece is up: she walks back to look at it, ending turned to it, and says so
                if out {
                    let walked = {
                        let mut comp = shared.companion.lock().unwrap_or_else(|e| e.into_inner());
                        comp.go_stay(to, ms).map(|m| (m, comp.moves))
                    };
                    if let Some(((from, there, d), id)) = walked {
                        w.send_raw(dojo::SMSG_MONSTER_MOVE, &dojo::walk_facing_body(companion::GUID, from, there, id, d, to.o))?;
                    }
                    monster_say(&w, if ring_job { dojo::BUILT_DOJO } else { dojo::BUILT })?;
                }
                shared.event("dojo", "", "admire");
            }
            dojo::Step::Done { shown, admired } => {
                shared.companion.lock().unwrap_or_else(|e| e.into_inner()).follow_on();
                jaina_feed(shared, crate::jaina::Event::Built);
                let (title, ring) = done.clone().unwrap_or_default();
                shared.event("dojo", "", &format!("built {shown}"));
                if ring {
                    dummies_up(shared, p, 3);
                }
                if out && !admired {
                    monster_say(&w, if ring { dojo::BUILT_DOJO } else { dojo::BUILT })?;
                }
                notify_host(shared, Item::line("ok", format!("{title} is up ({shown} props). .build undo takes it back.")))?;
                let line = shared.dojo.lock().unwrap_or_else(|e| e.into_inner()).line() + &sandbox::line_extra(shared);
                bar_data(&w, shared, sess_id, PLAYER_GUID, "dojo", &line)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_at(b: &[u8], i: usize) -> f32 {
        f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
    }

    /// The player's own create, byte for byte against AzerothCore (Object::BuildCreateUpdateBlockForPlayer,
    /// Object::BuildMovementUpdate, Unit::BuildMovementPacket): CREATE_OBJECT2, the packed guid, type player, update
    /// flags SELF | LIVING | STATIONARY_POSITION, movement flags 0 + flags2 0, the server's clock, x y z o, fall time 0,
    /// then the nine speeds in AzerothCore's order (walk, run, run back, swim, swim back, flight, flight back, turn,
    /// pitch), then the values.
    #[test]
    fn the_players_own_create_is_azerothcores_byte_for_byte() {
        let me = Me { guid: PLAYER_GUID, name: "Wren".into(), gender: HeroGender::Female, host: true };
        let p = At { x: 16226.2, y: 16257.0, z: 13.25, o: 1.5 };
        let mut v = Vec::new();
        player_create(&me, p, true).write_unencrypted_server(&mut v).unwrap();
        // header: u16 size (big endian), u16 opcode
        assert_eq!(u16::from_le_bytes([v[2], v[3]]), 0x00A9);
        let b = &v[4..];
        assert_eq!(&b[..4], &1u32.to_le_bytes(), "one object");
        assert_eq!(b[4], 3, "UPDATETYPE_CREATE_OBJECT2 (a player)");
        assert_eq!(&b[5..7], &[0x01, 0x01], "packed guid 1");
        assert_eq!(b[7], 4, "TYPEID_PLAYER");
        assert_eq!(u16::from_le_bytes([b[8], b[9]]), 0x0061, "SELF | LIVING | STATIONARY_POSITION");
        assert_eq!(&b[10..16], &[0; 6], "movement flags and flags2: none (no root, no gravity off)");
        let t = u32::from_le_bytes([b[16], b[17], b[18], b[19]]);
        assert!(t >= 1000, "the server's clock, not 0: {t}");
        assert_eq!([f32_at(b, 20), f32_at(b, 24), f32_at(b, 28), f32_at(b, 32)], [16226.2, 16257.0, 13.25, 1.5]);
        assert_eq!(&b[36..40], &[0; 4], "fall time");
        let speeds: Vec<f32> = (0..9).map(|k| f32_at(b, 40 + 4 * k)).collect();
        assert_eq!(speeds, vec![2.5, 7.0, 4.5, 4.722222, 2.5, 7.0, 4.5, std::f32::consts::PI, std::f32::consts::PI]);
        // no position block after Living, no high guid / target / transport / vehicle / rotation: the values next
        let blocks = b[76] as usize;
        assert!((5..=60).contains(&blocks), "the update mask's block count: {blocks}");
        // someone else's copy of a player: the same without SELF
        let mut o = Vec::new();
        player_create(&me, p, false).write_unencrypted_server(&mut o).unwrap();
        assert_eq!(u16::from_le_bytes([o[12], o[13]]), 0x0060);
    }
}
