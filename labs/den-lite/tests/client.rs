//! A headless 3.3.5a test client built from the same message crates: it logs in (SRP6), reads the realm list,
//! enters the world, checks the player create and the room's objects, walks, puts a stamp down, queries an object,
//! saves the Den to a file, takes the stamp back, opens the file again (empty, then replacing, then undo) and logs out.
//!
//! What this proves and what it doesn't: the server and this client agree with each other and with the crates'
//! idea of the 3.3.5a wire format. A stock game client can't run here, so what it accepts is "to verify" (README).

use den_lite::den::{self, Spot};
use den_lite::{start, Config, Gender as HeroGender, Hero};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;
use wow_login_messages::all::{Locale, Os, Platform, ProtocolVersion, Version, CMD_AUTH_LOGON_CHALLENGE_Client};
use wow_login_messages::version_8::opcodes::ServerOpcodeMessage as LoginServer;
use wow_login_messages::version_8::{
    CMD_AUTH_LOGON_CHALLENGE_Server, CMD_AUTH_LOGON_PROOF_Client, CMD_AUTH_LOGON_PROOF_Client_SecurityFlag, CMD_AUTH_LOGON_PROOF_Server,
    CMD_REALM_LIST_Client,
};
use wow_login_messages::Message as LoginMessage;
use wow_srp::client::SrpClientChallenge;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::{ClientCrypto, ProofSeed};
use wow_srp::PublicKey;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::wrath::*;
use wow_world_messages::Guid;

const ADDON_INFO: u16 = 0x02EF;
const GO_QUERY_RESPONSE: u16 = 0x005F;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stamps() -> PathBuf {
    repo().join("realm/custom/lua_scripts/den-stamps")
}

fn server(tag: &str, gender: HeroGender) -> (den_lite::Running, PathBuf) {
    server_with(tag, gender, false)
}

/// `companion`: at the player's side from login (the program's default; the older tests call her themselves).
fn server_with(tag: &str, gender: HeroGender, companion: bool) -> (den_lite::Running, PathBuf) {
    let dir = std::env::temp_dir().join(format!("den-lite-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config {
        login_port: 0,
        world_port: 0,
        password: "den".into(),
        hero: Hero { name: "Wren".into(), gender },
        den_dir: dir.clone(),
        stamps_dir: stamps(),
        companion,
        // no Den bar in these tests: the old lines come after a short wait; every encouragement moment, no gap
        ui_wait_ms: 300,
        ui_load_ms: 300,
        cheer_gap: 0,
        ..Config::default()
    };
    (start(cfg).expect("den lite starts"), dir)
}

fn lsend<M: LoginMessage>(s: &mut TcpStream, m: &M) {
    let mut v = Vec::new();
    m.write(&mut v).unwrap();
    s.write_all(&v).unwrap();
}

/// Logs in on the login server; returns the session key and the realm list's (name, address).
fn login(addr: SocketAddr, account: &str, password: &str) -> Result<([u8; 40], String, String), String> {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    lsend(
        &mut s,
        &CMD_AUTH_LOGON_CHALLENGE_Client {
            protocol_version: ProtocolVersion::Eight,
            version: Version { major: 3, minor: 3, patch: 5, build: 12340 },
            platform: Platform::X86,
            os: Os::Windows,
            locale: Locale::EnUs,
            utc_timezone_offset: 0,
            client_ip_address: Ipv4Addr::LOCALHOST,
            account_name: account.to_uppercase(),
        },
    );
    let LoginServer::CMD_AUTH_LOGON_CHALLENGE(CMD_AUTH_LOGON_CHALLENGE_Server::Success { generator, large_safe_prime, salt, server_public_key, .. }) =
        LoginServer::read(&mut s).unwrap()
    else {
        return Err("challenge failed".into());
    };
    let user = NormalizedString::new(account).unwrap();
    let c = SrpClientChallenge::new(
        user,
        NormalizedString::new(password).unwrap(),
        generator[0],
        large_safe_prime.try_into().unwrap(),
        PublicKey::from_le_bytes(server_public_key).unwrap(),
        salt,
    );
    lsend(
        &mut s,
        &CMD_AUTH_LOGON_PROOF_Client {
            client_public_key: *c.client_public_key(),
            client_proof: *c.client_proof(),
            crc_hash: [0; 20],
            telemetry_keys: vec![],
            security_flag: CMD_AUTH_LOGON_PROOF_Client_SecurityFlag::empty(),
        },
    );
    let server_proof = match LoginServer::read(&mut s).unwrap() {
        LoginServer::CMD_AUTH_LOGON_PROOF(CMD_AUTH_LOGON_PROOF_Server::Success { server_proof, .. }) => server_proof,
        other => return Err(format!("proof refused: {other:?}")),
    };
    let client = c.verify_server_proof(server_proof).map_err(|_| "the server's proof is wrong")?;
    lsend(&mut s, &CMD_REALM_LIST_Client {});
    let LoginServer::CMD_REALM_LIST(list) = LoginServer::read(&mut s).unwrap() else { return Err("no realm list".into()) };
    assert_eq!(list.realms.len(), 1);
    Ok((*client.session_key(), list.realms[0].name.clone(), list.realms[0].address.clone()))
}

struct World {
    s: TcpStream,
    c: ClientCrypto,
    /// The middle-of-the-screen lines so far (SMSG_NOTIFICATION: the encouragement moments, cheers.rs).
    notes: Vec<String>,
}

/// What a server frame was: its opcode, its body, and the crate's reading of it (not for the two read by hand).
struct Frame {
    opcode: u16,
    body: Vec<u8>,
    msg: Option<ServerOpcodeMessage>,
}

fn reparse(opcode: u16, body: &[u8]) -> Option<ServerOpcodeMessage> {
    if opcode == ADDON_INFO || opcode == GO_QUERY_RESPONSE {
        return None; // the crate can't read SMSG_ADDON_INFO, and its query response has 6 data fields, not 24
    }
    if opcode == 0x0096 && body.get(1..5) == Some(&[0xFF; 4]) {
        return None; // an addon whisper (LANG_ADDON): the crate has no such language; read by hand (addon_text)
    }
    // the Dojo's messages, written by hand (dojo.rs): fly, speed, her casts, and her turning where she stands
    // (SMSG_MONSTER_MOVE with the facing-angle type): read by hand in the Dojo's test
    if matches!(opcode, 0x0E2 | 0x343 | 0x344 | 0x381 | 0x131 | 0x132) {
        return None;
    }
    // the editor's exact click (gizmo.rs): the one spell the player knows, and its silent answer, written by hand
    if matches!(opcode, 0x12A | 0x130 | 0x1DE) {
        return None;
    }
    if opcode == 0x00DD && body.get(1 + body.first().map_or(0, |m| m.count_ones() as usize) + 17) == Some(&4) {
        return None;
    }
    // the sandbox combat's messages (combat.rs, classes.rs), written by hand and read by hand in its test; the
    // values updates (health, power, the class's fields: SMSG_UPDATE_OBJECT of type 0) too
    // the companion's flights and curved walks (flight.rs: Flying / Catmullrom, every point whole) and her flying
    // switches, written by hand
    if opcode == 0x00DD {
        let n = 1 + body.first().map_or(0, |m| m.count_ones() as usize);
        if body.get(n + 17) == Some(&0) && body.get(n + 18..n + 22).is_some_and(|f| u32::from_le_bytes(f.try_into().unwrap()) & 0x42000 != 0) {
            return None;
        }
    }
    if matches!(opcode, 0x422 | 0x423 | 0x4D3 | 0x4D4) {
        return None;
    }
    if den_lite::combat::SERVER_OPS.contains(&opcode) || opcode == 0x0129 || (opcode == 0x00A9 && body.get(4) == Some(&0)) {
        return None;
    }
    let size = body.len() + 2;
    let mut v = Vec::with_capacity(size + 3);
    if size > 0x7FFF {
        let b = (size as u32).to_be_bytes();
        v.extend_from_slice(&[b[1] | 0x80, b[2], b[3]]);
    } else {
        v.extend_from_slice(&(size as u16).to_be_bytes());
    }
    v.extend_from_slice(&opcode.to_le_bytes());
    v.extend_from_slice(body);
    Some(ServerOpcodeMessage::read_unencrypted(&v[..]).unwrap_or_else(|e| panic!("opcode {opcode:#x} doesn't read: {e}")))
}

impl World {
    fn connect(addr: &str, account: &str, key: [u8; 40], addons: u32) -> World {
        let mut s = TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut h = [0u8; 4];
        s.read_exact(&mut h).unwrap();
        let size = u16::from_be_bytes([h[0], h[1]]) as usize;
        let mut body = vec![0u8; size - 2];
        s.read_exact(&mut body).unwrap();
        let Some(ServerOpcodeMessage::SMSG_AUTH_CHALLENGE(ch)) = reparse(u16::from_le_bytes([h[2], h[3]]), &body) else {
            panic!("no auth challenge")
        };
        let seed = ProofSeed::new();
        let client_seed = seed.seed();
        let user = NormalizedString::new(account).unwrap();
        let (proof, c) = seed.into_client_header_crypto(&user, key, ch.server_seed);
        let auth = CMSG_AUTH_SESSION {
            client_build: 12340,
            login_server_id: 0,
            username: account.to_uppercase(),
            login_server_type: 0,
            client_seed,
            region_id: 0,
            battleground_id: 0,
            realm_id: 1,
            dos_response: 0,
            client_proof: proof,
            addon_info: addon_blob(addons),
        };
        let mut v = Vec::new();
        auth.write_unencrypted_client(&mut v).unwrap();
        s.write_all(&v).unwrap();
        World { s, c, notes: Vec::new() }
    }

    fn frame(&mut self) -> Frame {
        let h = self.c.read_and_decrypt_server_header(&mut self.s).expect("a frame");
        let mut body = vec![0u8; h.size as usize - 2];
        self.s.read_exact(&mut body).unwrap();
        let msg = reparse(h.opcode, &body);
        if let Some(ServerOpcodeMessage::SMSG_NOTIFICATION(n)) = &msg {
            self.notes.push(n.notification.clone());
        }
        Frame { opcode: h.opcode, msg, body }
    }

    fn send<M: ClientMessage>(&mut self, m: &M) {
        let mut v = Vec::new();
        m.write_encrypted_client(&mut v, self.c.encrypter()).unwrap();
        self.s.write_all(&v).unwrap();
    }

    /// A frame the crate can't write: an addon whisper to ourselves (SendAddonMessage(prefix, text, "WHISPER", me)).
    fn addon(&mut self, prefix: &str, text: &str) {
        let mut body = Vec::new();
        body.extend_from_slice(&7u32.to_le_bytes());
        body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        body.extend_from_slice(b"Wren\0");
        body.extend_from_slice(format!("{prefix}\t{text}").as_bytes());
        body.push(0);
        let mut v = Vec::new();
        self.c.encrypter().write_encrypted_client_header(&mut v, (body.len() + 4) as u16, 0x0095).unwrap();
        v.extend_from_slice(&body);
        self.s.write_all(&v).unwrap();
    }

    /// A client frame written by hand (the Dojo's acks, as the game sends them).
    fn raw(&mut self, opcode: u32, body: &[u8]) {
        let mut v = Vec::new();
        self.c.encrypter().write_encrypted_client_header(&mut v, (body.len() + 4) as u16, opcode).unwrap();
        v.extend_from_slice(body);
        self.s.write_all(&v).unwrap();
    }

    /// The next addon answer for `md`/`name`, its parts joined.
    fn until_addon(&mut self, md: &str, name: &str) -> String {
        let want = format!("OVMOD\t{md}\t{name}\t");
        let mut parts: Vec<String> = Vec::new();
        loop {
            let f = self.frame();
            let Some(t) = addon_text(&f) else { continue };
            let Some(rest) = t.strip_prefix(&want) else { continue };
            let (pn, data) = rest.split_once('\t').unwrap();
            let (i, n) = pn.split_once('/').unwrap();
            let (i, n): (usize, usize) = (i.parse().unwrap(), n.parse().unwrap());
            assert_eq!(i, parts.len() + 1, "parts in order");
            parts.push(data.to_string());
            if i == n {
                return parts.concat();
            }
        }
    }

    fn say(&mut self, text: &str) {
        self.send(&CMSG_MESSAGECHAT { chat_type: CMSG_MESSAGECHAT_ChatType::Say, language: Language::Common, message: text.into() });
    }

    /// Frames until one matches (the ones before it are skipped).
    fn until<T>(&mut self, mut want: impl FnMut(&Frame) -> Option<T>) -> T {
        loop {
            let f = self.frame();
            if let Some(t) = want(&f) {
                return t;
            }
        }
    }

    /// The next line the companion says (monster say): who says it and what.
    fn until_she_says(&mut self) -> (u64, String, String) {
        self.until(|f| match &f.msg {
            Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) => match &m.chat_type {
                SMSG_MESSAGECHAT_ChatType::MonsterSay { sender1, .. } => Some((m.sender.guid(), sender1.clone(), m.message.clone())),
                _ => None,
            },
            _ => None,
        })
    }

    /// Frames until a system chat line arrives (the end of every command's answer); returns them and the line.
    fn until_line(&mut self) -> (Vec<Frame>, String) {
        let mut got = Vec::new();
        loop {
            let f = self.frame();
            if let Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) = &f.msg {
                if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::System { .. }) {
                    return (got, m.message.clone());
                }
            }
            got.push(f);
        }
    }
}

/// An addon whisper's text (SMSG_MESSAGECHAT, LANG_ADDON): u8 type, u32 lang, u64 sender, u32 flags, u64 target,
/// u32 length, the text.
fn addon_text(f: &Frame) -> Option<String> {
    if f.opcode != 0x0096 || f.body.get(1..5) != Some(&[0xFF; 4]) || f.body.len() < 30 {
        return None;
    }
    let len = u32::from_le_bytes(f.body[25..29].try_into().unwrap()) as usize;
    Some(String::from_utf8_lossy(&f.body[29..29 + len - 1]).to_string())
}

/// The CMSG_AUTH_SESSION addon list: u32 size, then zlib (u32 count, each: name, has key, crc, extra crc; a time).
fn addon_blob(n: u32) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    let mut plain = Vec::new();
    plain.extend_from_slice(&n.to_le_bytes());
    for i in 0..n {
        plain.extend_from_slice(format!("Leyline_Addon{i}").as_bytes());
        plain.push(0);
        plain.push(1);
        plain.extend_from_slice(&0x4C1C_776Du32.to_le_bytes());
        plain.extend_from_slice(&0u32.to_le_bytes());
    }
    plain.extend_from_slice(&0u32.to_le_bytes());
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&plain).unwrap();
    let mut out = (plain.len() as u32).to_le_bytes().to_vec();
    out.extend(z.finish().unwrap());
    out
}

/// A gameobject the client was told about.
#[derive(Debug, Clone, PartialEq)]
struct Go {
    entry: i32,
    display: i32,
    scale: f32,
    x: f32,
    y: f32,
    z: f32,
    o: f32,
    rotation: u64,
}

/// The gameobject creates and destroys in some frames.
fn objects(frames: &[Frame]) -> (HashMap<u64, Go>, Vec<u64>) {
    let mut made = HashMap::new();
    let mut gone = Vec::new();
    for f in frames {
        match &f.msg {
            Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => {
                for o in &u.objects {
                    if let Object::CreateObject { guid3, mask2: UpdateMask::GameObject(g), movement2, object_type: ObjectType::GameObject } = o {
                        let Some(MovementBlock_UpdateFlag_Living::HasPosition { orientation2, position2 }) = movement2.update_flag.get_living() else {
                            panic!("a gameobject without its position")
                        };
                        let rotation = movement2.update_flag.get_rotation().expect("a rotation").packed_local_rotation;
                        // 0x10: AzerothCore's UPDATEFLAG_LOWGUID, the crate's (mangos') HIGH_GUID: the guid's counter
                        assert_eq!(movement2.update_flag.get_high_guid().map(|h| h.unknown0 as u64), Some(guid3.guid() & 0xFF_FFFF));
                        assert_eq!(g.object_guid(), Some(*guid3));
                        made.insert(
                            guid3.guid(),
                            Go {
                                entry: g.object_entry().unwrap(),
                                display: g.gameobject_displayid().unwrap(),
                                scale: g.object_scale_x().unwrap(),
                                x: position2.x,
                                y: position2.y,
                                z: position2.z,
                                o: *orientation2,
                                rotation,
                            },
                        );
                    }
                }
            }
            Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) => gone.push(d.guid.guid()),
            _ => {}
        }
    }
    (made, gone)
}

fn heartbeat(x: f32, y: f32, z: f32, o: f32) -> MSG_MOVE_HEARTBEAT {
    MSG_MOVE_HEARTBEAT {
        guid: Guid::new(den_lite::world::PLAYER_GUID),
        info: MovementInfo {
            flags: MovementInfo_MovementFlags::empty(),
            timestamp: 1000,
            position: Vector3d { x, y, z },
            orientation: o,
            fall_time: 0.0,
        },
    }
}

#[test]
fn a_player_logs_in_builds_saves_and_opens_their_den() {
    let (srv, dir) = server("full", HeroGender::Male);
    // the launcher fills in the account DEN
    let (key, realm, address) = login(srv.login, "DEN", "den").expect("login");
    assert_eq!(realm, "Your Den");
    assert_eq!(address, srv.world.to_string());

    let mut w = World::connect(&address, "DEN", key, 2);
    // the auth session's answer, in AzerothCore's order
    let f = w.frame();
    assert!(matches!(f.msg, Some(ServerOpcodeMessage::SMSG_AUTH_RESPONSE(ref r)) if matches!(**r, SMSG_AUTH_RESPONSE::AuthOk { .. })));
    let f = w.frame();
    assert_eq!(f.opcode, ADDON_INFO);
    assert_eq!(f.body.len(), 2 * 8 + 4, "two addons (8 bytes each) and no banned ones");
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_CLIENTCACHE_VERSION(_))));
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_TUTORIAL_FLAGS(_))));

    w.send(&CMSG_READY_FOR_ACCOUNT_DATA_TIMES {});
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_ACCOUNT_DATA_TIMES(_))));
    w.send(&CMSG_CHAR_ENUM {});
    let Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) = w.frame().msg else { panic!("no character list") };
    assert_eq!(e.characters.len(), 1);
    let ch = &e.characters[0];
    assert_eq!((ch.name.as_str(), ch.race, ch.gender, ch.map), ("Wren", Race::Human, Gender::Male, Map::Kalimdor));
    assert_eq!(ch.equipment[3].equipment_display_id, 7903, "the shirt");
    assert_eq!(ch.equipment[6].inventory_type, InventoryType::Legs);
    assert!((ch.position.x - 16226.2).abs() < 0.01 && (ch.position.y - 16257.0).abs() < 0.01);

    // into the world
    w.send(&CMSG_PLAYER_LOGIN { guid: ch.guid });
    let (frames, welcome) = w.until_line();
    assert!(welcome.starts_with("Welcome to your Den: 0 props"), "{welcome}");
    // the quick start: five lines, the first one places a chair, the last grows the room and calls the companion
    let tutorial: Vec<String> = (0..5).map(|_| w.until_line().1).collect();
    assert!(tutorial[0].contains(".den place chair") && tutorial[3].contains(".den save"), "{tutorial:?}");
    assert!(tutorial[4].contains(".den grow") && tutorial[4].contains("Jaina, tips") && tutorial[4].contains(".den companion"), "{tutorial:?}");
    let names: Vec<u16> = frames.iter().map(|f| f.opcode).collect();
    let at = |op: u16| names.iter().position(|&o| o == op).unwrap_or_else(|| panic!("no {op:#x} in {names:x?}"));
    // SMSG_LOGIN_VERIFY_WORLD first, then account data, ..., the player, world states, the room
    assert_eq!(at(0x0236), 0);
    assert!(at(0x0209) < at(0x00A9), "account data times before the player create");
    // the time sync after the player exists (AzerothCore's SendInitialPacketsAfterAddToMap; 3 Nov: it went before)
    assert!(at(0x0390) > at(0x00A9), "time sync after the player create");
    assert!(at(0x00A9) < at(0x02C2), "the player before the world states");
    let Some(ServerOpcodeMessage::SMSG_LOGIN_VERIFY_WORLD(v)) = &frames[0].msg else { unreachable!() };
    assert_eq!(v.map, Map::Kalimdor);
    let player = frames
        .iter()
        .filter_map(|f| match &f.msg {
            Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => u.objects.iter().find_map(|o| match o {
                Object::CreateObject2 { mask2: UpdateMask::Player(p), movement2, .. } => Some((p.clone(), movement2.clone())),
                _ => None,
            }),
            _ => None,
        })
        .next()
        .expect("the player's create");
    assert!(player.1.update_flag.get_self());
    // AzerothCore's flags for a player's own create (SELF | LIVING | STATIONARY_POSITION) and the server's clock: the
    // bytes are checked in world.rs's own test; here the crate still reads the create as a living self
    assert_eq!(player.0.unit_displayid(), Some(49));
    assert!(player.0.player_visible_item(VisibleItemIndex::Index3).is_some(), "the shirt shows");
    let Some(MovementBlock_UpdateFlag_Living::Living { position, running_speed, timestamp, .. }) = player.1.update_flag.get_living() else {
        panic!("the player isn't living")
    };
    assert!(*timestamp >= 1000, "the server's clock in the movement block, not 0: {timestamp}");
    // on the ground, not a yard above it (3 Nov: a game that didn't move its character showed it floating)
    assert!((position.z - (13.2 + den_lite::movewatch::SPAWN_LIFT)).abs() < 0.01 && *running_speed == 7.0, "{}", position.z);
    // no control update during the loading screen (AzerothCore sends none at login): only once the game takes it
    assert!(!names.contains(&0x0159), "no control update before the game takes its character");
    assert!(names.contains(&0x02C2), "SMSG_INIT_WORLD_STATES");
    let (room, _) = objects(&frames);
    // a new Den is the open white plane: no room pieces (owner 1 Nov 20:55), the launcher's white ground is the floor
    assert_eq!(room.len(), 0, "no ice-block room");
    // a room is still there to choose: stone
    w.say(".den room stone");
    let (frames, _) = w.until_line();
    let (room, _) = objects(&frames);
    assert_eq!(room.values().filter(|g| g.entry == 180043).count(), 4, "its lights");
    // east and west walls are turned a quarter: their packed rotation carries z = sin(45 degrees)
    assert!(room.values().any(|g| (g.o - std::f32::consts::FRAC_PI_2).abs() < 1e-4 && g.rotation == (0.70710677f32 * (1 << 20) as f32) as u64));
    w.say(".den room white");
    let (frames, _) = w.until_line();
    let (_, gone) = objects(&frames);
    assert!(!gone.is_empty(), "back to the open plane: the stone room goes");

    // walk 3 yards east, 2 north, facing east; then a stamp at our feet
    let (px, py, pz) = (16226.2 + 3.0, 16257.0 + 2.0, 14.2);
    // the game says it took its character (as the real one does once the world is on screen): told once more that it
    // has control, and unrooted
    w.send(&CMSG_SET_ACTIVE_MOVER { guid: Guid::new(den_lite::world::PLAYER_GUID) });
    let ctl = w.until(|f| (f.opcode == 0x0159).then(|| f.body.clone()));
    assert_eq!(ctl, den_lite::movewatch::control_body(den_lite::world::PLAYER_GUID));
    let unroot = w.until(|f| (f.opcode == 0x00EA).then(|| f.body.clone()));
    assert_eq!(unroot, den_lite::movewatch::unroot_body(den_lite::world::PLAYER_GUID, 0));
    // the game answers the time sync, then walks: MSG_MOVE_START_FORWARD as the real game sends it (packed guid,
    // flags FORWARD, flags2, its clock, x y z o, fall time), then a heartbeat further on
    let mut sync = 0u32.to_le_bytes().to_vec();
    sync.extend_from_slice(&5000u32.to_le_bytes());
    w.raw(0x391, &sync);
    let mut fwd = vec![0x01, 0x01];
    fwd.extend_from_slice(&den_lite::movewatch::FORWARD.to_le_bytes());
    fwd.extend_from_slice(&0u16.to_le_bytes());
    fwd.extend_from_slice(&5100u32.to_le_bytes());
    for f in [16226.2f32, 16257.0, 13.25, 0.0, 0.0] {
        fwd.extend_from_slice(&f.to_le_bytes());
    }
    w.raw(0x0B5, &fwd);
    w.send(&heartbeat(px, py, pz, 0.0));
    // where we stand against the ground, and what the server saw of our moves
    w.say(".den where");
    let (_, line) = w.until_line();
    assert!(line.contains("time sync answered") && line.contains("MSG_MOVE_START_FORWARD x1"), "{line}");
    assert!(line.starts_with("You stand at 16229.2, 16259.0, 14.20: 1.00 above the ground (the ground 13.20, the fixed floor)."), "{line}");
    assert!(line.contains("the game took its character") && line.contains("walked (3.6 yards)"), "{line}");
    let ev = den_lite::events::read(&dir);
    let detail = |name: &str| ev.iter().find(|l| l.ev == name).map(|l| l.detail.clone()).unwrap_or_else(|| panic!("no {name} in {ev:?}"));
    assert!(detail("spawn").contains("on the ground (+0.05), nearest prop none"), "{}", detail("spawn"));
    assert_eq!(detail("mover"), "the game moves its character");
    assert!(detail("first_move").starts_with("START_FORWARD, z 13.25, on the ground"), "{}", detail("first_move"));
    assert!(detail("time_sync").contains("counter 0, its clock 5000 ms"), "{}", detail("time_sync"));
    assert!(ev.iter().any(|l| l.ev == "client_op" && l.detail == "MSG_MOVE_START_FORWARD"), "every opcode written down");
    assert!(detail("walked").starts_with("3.6 yards by HEARTBEAT"), "{}", detail("walked"));
    // the camp stamp (tools/den-stamps/compose.py): its fire where you stand, benches round it facing it
    let n = den::read_stamp(&dir.join("no-stamps-here"), "campfire").expect("the built-in camp").props.len();
    assert!((10..=40).contains(&n), "{n}");
    w.say(".den stamp campfire now");
    let (frames, line) = w.until_line();
    assert!(line.starts_with(&format!("Camp is down ({n} props)")), "{line}");
    let (stamp, _) = objects(&frames);
    assert_eq!(stamp.len(), n);
    let fire = stamp.values().find(|g| g.entry == 1953).expect("the campfire");
    assert_eq!(fire.display, 192);
    assert!((fire.x - px).abs() < 0.01 && (fire.y - py).abs() < 0.01 && (fire.z - pz).abs() < 0.01);
    assert!(stamp.values().any(|g| {
        let d = ((g.x - px).powi(2) + (g.y - py).powi(2)).sqrt();
        let to_fire = (py - g.y).atan2(px - g.x).rem_euclid(std::f32::consts::TAU);
        let off = (g.o - to_fire + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        g.entry == 24538 && (2.6..3.4).contains(&d) && off.abs() < 0.3
    }), "a bench about 3 yards from the fire, facing it");
    let stamp_guids: Vec<u64> = stamp.keys().copied().collect();

    // one piece by name, 3 yards ahead (facing east), then taken back
    w.say(".den find chair");
    let (_, line) = w.until_line();
    assert!(line.starts_with("Wooden Chair") || line.contains("Wooden Chair"), "{line}");
    w.say(".den place wooden chair");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("Wooden Chair is down (1 props)"), "{line}");
    let (one, _) = objects(&frames);
    let chair = one.values().next().expect("the chair");
    assert_eq!((chair.entry, chair.display), (2413, 39));
    assert!((chair.x - (px + 3.0)).abs() < 0.01 && (chair.y - py).abs() < 0.01);
    w.say(".den place nosuchthing");
    assert!(w.until_line().1.starts_with("Nothing called 'nosuchthing'"));
    w.say(".build undo");
    let (frames, _) = w.until_line();
    assert_eq!(objects(&frames).1.len(), 1, "the chair goes");

    // the client asks what a campfire is
    w.send(&CMSG_GAMEOBJECT_QUERY { entry_id: 1953, guid: Guid::new(stamp_guids[0]) });
    let f = w.frame();
    assert_eq!(f.opcode, GO_QUERY_RESPONSE);
    assert_eq!(&f.body[0..12], &[1953u32.to_le_bytes(), 8u32.to_le_bytes(), 192u32.to_le_bytes()].concat()[..]);
    assert_eq!(&f.body[12..21], b"Campfire\0");
    assert_eq!(f.body.len(), 12 + 9 + 6 + 24 * 4 + 4 + 6 * 4);

    // save, undo, load
    w.say(".den save test1");
    let (_, line) = w.until_line();
    assert!(line.starts_with(&format!("Saved to a file ({n} props)")), "{line}");
    let saved = den::read_file(&dir.join("test1.den.json")).expect("a Den file");
    assert_eq!(saved.props.len(), n);
    let f = saved.props.iter().find(|p| p.entry == 1953).unwrap();
    assert!((f.x - 3.0).abs() < 0.01 && (f.y - 2.0).abs() < 0.01 && (f.z - 1.0).abs() < 0.01);
    assert_eq!(saved.room.as_ref().unwrap().look, "white");

    w.say(".build undo");
    let (frames, line) = w.until_line();
    let (_, gone) = objects(&frames);
    assert_eq!(gone.len(), n, "{line}");
    // the encouragement moments (cheers.rs): first visit, first piece, first save, each once, never in the chat frame
    assert_eq!(w.notes.len(), 3, "{:?}", w.notes);
    assert_eq!(w.notes.iter().map(|n| den_lite::cheers::LINES.iter().position(|(_, l)| l == n)).collect::<Vec<_>>(),
               vec![Some(0), Some(1), Some(2)], "{:?}", w.notes);
    w.say(".den save test2");
    w.until_line();
    w.say(".den files");
    w.until_line();
    assert_eq!(w.notes.len(), 3, "a second save says nothing more");
    let _ = std::fs::remove_file(dir.join("test2.den.json"));
    assert!(stamp_guids.iter().all(|g| gone.contains(g)));

    w.say(".den load test1");
    let (frames, line) = w.until_line();
    assert_eq!(line, format!("Opened: {n} props."));
    let (loaded, _) = objects(&frames);
    let mut a: Vec<(i32, i32, i32)> = stamp.values().map(|g| (g.entry, (g.x * 100.0) as i32, (g.y * 100.0) as i32)).collect();
    let mut b: Vec<(i32, i32, i32)> = loaded.values().map(|g| (g.entry, (g.x * 100.0) as i32, (g.y * 100.0) as i32)).collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "the file brings back the same props in the same places");

    w.say(".den load test1");
    let (_, line) = w.until_line();
    assert!(line.contains("replace") && line.contains("add"), "{line}");
    w.say(".den load test1 replace");
    let (frames, _) = w.until_line();
    let (made, gone) = objects(&frames);
    assert_eq!((made.len(), gone.len()), (n, n));
    assert!(dir.join("backup.den.json").exists(), "a copy is kept as backup");
    w.say(".build undo");
    let (frames, line) = w.until_line();
    let (made, gone) = objects(&frames);
    assert_eq!((made.len(), gone.len()), (n, n), "{line}");
    assert_eq!(srv.shared.den.lock().unwrap().props.len(), n);

    // a look, other questions, and out
    w.say(".den room stone");
    let (frames, line) = w.until_line();
    let (made, gone) = objects(&frames);
    assert_eq!(line, "The room is stone now.");
    assert_eq!(gone.len(), 0, "from the open plane: nothing to take down");
    assert!(made.values().all(|g| [190024, 19444, 180043].contains(&g.entry)));
    // the launcher's Saved Dens > Open, while the player is in: open.json (host/dens.rs open_doc)
    std::fs::write(dir.join("open.json"), r#"{"file":"test1","id":"18f3a9c2b7e1d4a51234"}"#).unwrap();
    let (_, line) = w.until_line();
    assert_eq!(line, "Opening 'test1' from the launcher.");
    let (frames, line) = w.until_line();
    assert_eq!(line, format!("Opened: {n} props."));
    assert_eq!(objects(&frames).1.len(), n + 34, "what was there goes, and the file's white room comes back for the stone one");
    assert!(std::fs::read_to_string(dir.join(".open-done")).unwrap().contains("18f3a9c2b7e1d4a51234"));
    assert!(!srv.shared.mark_opened(&dir, "18f3a9c2b7e1d4a51234"), "each id opens once");
    w.send(&CMSG_PING { sequence_id: 7, round_time_in_ms: 0 });
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_PONG(SMSG_PONG { sequence_id: 7 }))));
    w.send(&CMSG_NAME_QUERY { guid: ch.guid });
    let Some(ServerOpcodeMessage::SMSG_NAME_QUERY_RESPONSE(n)) = w.frame().msg else { panic!("no name") };
    assert_eq!(n.character_name, "Wren");
    w.send(&CMSG_LOGOUT_REQUEST {});
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_LOGOUT_RESPONSE(_))));
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_LOGOUT_COMPLETE)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_wrong_password_is_refused_and_any_name_works() {
    let (srv, dir) = server("pw", HeroGender::Female);
    assert!(login(srv.login, "someone", "nope").is_err());
    assert!(login(srv.login, "someone", "den").is_ok());
    assert!(login(srv.login, "another_one", "den").is_ok());
    // a world connection without a login is told so
    let (key, _, address) = login(srv.login, "third", "den").unwrap();
    let mut w = World::connect(&address, "third", key, 0);
    assert!(matches!(w.frame().msg, Some(ServerOpcodeMessage::SMSG_AUTH_RESPONSE(_))));
    w.send(&CMSG_CHAR_ENUM {});
    // skip addon info, cache version, tutorials
    let e = loop {
        if let Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) = w.frame().msg {
            break e;
        }
    };
    assert_eq!(e.characters[0].gender, Gender::Female);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_stamp_and_starter_opens_with_nothing_left_out() {
    let spot = Spot::default();
    for name in den::list_names(&stamps()) {
        let st = den::read_stamp(&stamps(), &name).unwrap();
        let mut d = den::Den::new(spot);
        let c = d.stamp(&st, spot.x, spot.y, spot.z, 0.0);
        assert_eq!(c.skipped, 0, "{name}");
        assert_eq!(c.created.len(), st.props.len(), "{name}");
    }
    for name in ["campfire", "dock", "garden", "house", "inn", "market"] {
        assert!(den::read_stamp(std::path::Path::new("/no/such/folder"), name).is_ok(), "{name} is built in");
    }
    let starters = repo().join("realm/custom/lua_scripts/den-starters");
    for name in den::list_names(&starters) {
        let doc = den::read_file(&starters.join(format!("{name}.den.json"))).unwrap();
        let mut d = den::Den::new(spot);
        let c = d.open(&doc, true);
        assert_eq!(c.skipped, 0, "{name}");
        // and back out to a file with the same props
        assert_eq!(d.to_doc(&name, 0).props.len(), doc.props.len());
    }
}

/// Logs in and enters the world; the frames up to the quick start are read.
fn into_world(srv: &den_lite::Running, addons: u32) -> World {
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, addons);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    for _ in 0..6 {
        w.until_line();
    }
    w
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

#[test]
fn the_den_grows_and_the_companion_talks_with_the_assistant() {
    let (srv, dir) = server("grow", HeroGender::Female);
    let mut w = into_world(&srv, 2);
    let (mx, my, mz) = (16226.2f32, 16257.0f32, 14.2f32);

    // 1. the room grows by rings, is saved with its size and opens at it again
    w.say(".den size");
    assert!(w.until_line().1.starts_with("Your Den is size 1 of 7: a 16 x 16 yard room"));
    w.say(".den grow");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("Your Den is size 2 of 7: a 32 x 32 yard room, props up to 30 yards"), "{line}");
    let (made, gone) = objects(&frames);
    assert_eq!((made.len(), gone.len()), (0, 0), "the open plane: only the size changes");
    let _ = mx;
    // a chair 25 yards out fits now (it didn't at size 1)
    w.send(&heartbeat(mx + 22.0, my, mz, 0.0));
    w.say(".den place wooden chair");
    assert!(w.until_line().1.starts_with("Wooden Chair is down"));
    assert!(w.notes.iter().any(|n| n == den_lite::cheers::LINES[3].1), "the first grow: one line ({:?})", w.notes);
    w.say(".den size 1");
    assert!(w.until_line().1.starts_with("1 props stand outside a size 1 Den"));
    w.say(".den save grown");
    w.until_line();
    assert_eq!(den::read_file(&dir.join("grown.den.json")).unwrap().room.unwrap().size, 16.0);
    w.say(".build undo");
    w.until_line();
    w.say(".den size 1");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("Your Den is size 1 of 7"), "{line}");
    assert_eq!(objects(&frames).0.len(), 0);
    w.say(".den load grown");
    let (frames, line) = w.until_line();
    assert_eq!(line, "Opened: 1 props.");
    assert_eq!(objects(&frames).0.len(), 1, "the file's chair (the white Den has no room pieces)");
    assert_eq!(srv.shared.den.lock().unwrap().spot.half, 16.0);
    w.say(".den size 9");
    assert!(w.until_line().1.starts_with("Sizes are 1 to 7"));

    // 2. the companion: called, named, heard, answering from the assistant's file, following, staying in the Den
    w.send(&heartbeat(mx, my, mz, 0.0));
    w.say(".den ask a well");
    assert_eq!(w.until_line().1, "Call your companion first: .den companion");
    w.say(".den companion");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("Your companion follows you."), "{line}");
    let unit = frames
        .iter()
        .find_map(|f| match &f.msg {
            Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => u.objects.iter().find_map(|o| match o {
                Object::CreateObject { guid3, mask2: UpdateMask::Unit(m), movement2, object_type: ObjectType::Unit } => {
                    Some((guid3.guid(), m.clone(), movement2.clone()))
                }
                _ => None,
            }),
            _ => None,
        })
        .expect("her create");
    assert_eq!(unit.0, den_lite::companion::GUID);
    assert_eq!((unit.1.object_entry(), unit.1.unit_displayid(), unit.1.unit_factiontemplate(), unit.1.unit_flags()), (Some(32364), Some(30865), Some(35), Some(832)));
    let Some(MovementBlock_UpdateFlag_Living::Living { position, .. }) = unit.2.update_flag.get_living() else { panic!("she isn't living") };
    assert!(((position.x - mx).powi(2) + (position.y - my).powi(2)).sqrt() - 2.5 < 0.01, "2.5 yards from the player");
    let hello = frames.iter().find_map(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::MonsterSay { .. }) => Some(m.message.clone()),
        _ => None,
    });
    assert!(hello.unwrap().starts_with("Hello!"));
    // the client asks who she is
    w.send(&CMSG_CREATURE_QUERY { creature: 32364, guid: Guid::new(den_lite::companion::GUID) });
    let q = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CREATURE_QUERY_RESPONSE(r)) => Some(r.clone()),
        _ => None,
    });
    let found = q.found.expect("known");
    assert_eq!((found.name1.as_str(), found.sub_name.as_str(), found.display_ids[0], found.creature_type), ("Jaina", "Your companion", 30865, 7));
    // "Jaina, ..." near her: said aloud, and written where den_messages reads it
    w.say("Jaina, a well by the door");
    let (who, name, text) = w.until_she_says();
    assert_eq!((who, name.as_str()), (den_lite::companion::GUID, "Jaina"));
    assert!(text.starts_with("I've written that down. Nobody is reading"), "nobody has read the asks yet: {text}");
    let cin: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("companion-in.json")).unwrap()).unwrap();
    assert_eq!((cin["format"].clone(), cin["server"].clone(), cin["companion"].clone()), (serde_json::json!("leyline-companion"), serde_json::json!("den-lite"), serde_json::json!(true)));
    assert_eq!(cin["asks"][0]["text"], "a well by the door");
    assert_eq!(cin["asks"][0]["n"], 1);
    assert_eq!(cin["asks"][0]["where"]["x"], 0.0);
    assert_eq!((cin["owner"]["online"].clone(), cin["room"]["size"].clone()), (serde_json::json!(true), serde_json::json!(2)));
    w.say(".den ask and a bench");
    assert_eq!(w.until_she_says().2, "One moment, I'm still on the last one.");
    // the assistant answers (den_say: launcher/src-tauri/src/agents/den.rs push_line) and reads the asks (read_at)
    let line = serde_json::json!({"format": "leyline-companion", "lines": [{"n": 1, "at": now(), "text": "A well, here? I'll draw one up."}],
        "seen": 1, "read_at": now()});
    std::fs::write(dir.join("companion-out.json"), line.to_string()).unwrap();
    assert_eq!(w.until_she_says().2, "A well, here? I'll draw one up.");
    std::thread::sleep(Duration::from_millis(4100));
    w.say(".den ask and a bench");
    assert_eq!(w.until_she_says().2, "Let me think about that.", "someone is listening now");
    // she follows: the player runs 12 yards east, she walks to her spot behind them
    w.send(&heartbeat(mx + 12.0, my, mz, 0.0));
    let mv = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MONSTER_MOVE(m)) => Some(m.clone()),
        _ => None,
    });
    assert_eq!(mv.guid.guid(), den_lite::companion::GUID);
    let to = mv.splines[0];
    let (sx, sy) = (mx + 12.0 + (0.75 * std::f32::consts::PI).cos() * 2.5, my + (0.75 * std::f32::consts::PI).sin() * 2.5);
    assert!((to.x - sx).abs() < 0.01 && (to.y - sy).abs() < 0.01, "{to:?}");
    assert!(mv.duration > 1000 && mv.duration < 3000, "{}", mv.duration);
    // far from the Den (past flight.rs AREA; she follows you anywhere nearer, 3 Nov): she stays
    w.send(&heartbeat(mx + 400.0, my, mz, 0.0));
    let line = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::System { .. }) => Some(m.message.clone()),
        _ => None,
    });
    assert!(line.starts_with("Your companion stays in your Den"), "{line}");
    // back inside: she joins again by herself, quietly (she speaks when spoken to); sent away, she stays away
    w.send(&heartbeat(mx, my, mz, 0.0));
    w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => u.objects.iter().any(|o| matches!(o, Object::CreateObject { guid3, .. } if guid3.guid() == den_lite::companion::GUID)).then_some(()),
        Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::MonsterSay { .. }) => panic!("she said {:?}", m.message),
        _ => None,
    });
    w.say(".den companion away");
    assert_eq!(w.until_she_says().2, den_lite::companion::BYE);
    w.say(".den companion away");
    assert_eq!(w.until_line().1, "Your companion isn't here.");

    // 3. game modes and mods: what Den Lite can say, and what it can't run
    w.say(".den mode");
    let first = w.until_line().1;
    assert!(first.starts_with("Host templates (run on this PC): levelling"), "{first}");
    w.until_line();
    w.until_line();
    w.say(".den mode minigolf");
    assert!(w.until_line().1.starts_with("Minigolf:"));
    assert!(w.until_line().1.contains("Minigolf night template"));
    assert!(w.until_line().1.contains("Leyline › Host › Host a mode › Minigolf night › Host"));
    w.say(".den mods");
    let l = w.until_line().1;
    assert!(l.starts_with("Your game loaded 2 addons: Leyline_Addon0, Leyline_Addon1"), "{l}");
    w.until_line();
    assert!(w.until_line().1.contains("Leyline launcher"), "no command-line flag shown to a player");
    w.send(&CMSG_LOGOUT_REQUEST {});
    w.until(|f| matches!(f.msg, Some(ServerOpcodeMessage::SMSG_LOGOUT_COMPLETE)).then_some(()));
    assert!(srv.shared.live.lock().unwrap().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Logs in on `login_addr` and enters the world; returns the client and every chat line up to the quick start's end.
fn into_world_at(login_addr: SocketAddr) -> (World, Vec<String>) {
    let (key, _, address) = login(login_addr, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 2);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    let lines = (0..6).map(|_| w.until_line().1).collect();
    (w, lines)
}

/// The real program (what the launcher's host/lite.rs runs, with its command line), on free ports.
struct Program(std::process::Child);

impl Drop for Program {
    /// Stopped also when a check fails, so no server is left running after the test.
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_program(dir: &std::path::Path, login_port: u16, world_port: u16) -> Program {
    let child = std::process::Command::new(env!("CARGO_BIN_EXE_den-lite"))
        .args(["--dens", &dir.display().to_string(), "--bind", "127.0.0.1", "--password", "den"])
        .args(["--stamps", &stamps().display().to_string()])
        .args(["--login-port", &login_port.to_string(), "--world-port", &world_port.to_string()])
        // this test's game sends no "past the loading screen" packet and has no Den bar: the old lines soon
        .args(["--ui-load", "500"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("den-lite runs");
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", login_port)).is_ok() {
            return Program(child);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("den-lite never answered on {login_port}");
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// The release smoke (docs/smoke/2026-11-01-den-host.md): a new player's first session on the real program, then the
/// program stopped and started again (the launcher closed, the PC restarted) and the saved Den opened.
#[test]
fn a_new_players_den_survives_a_restart_of_the_program() {
    let dir = std::env::temp_dir().join(format!("den-lite-test-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (lp, wp) = (free_port(), free_port());
    let login_addr: SocketAddr = ([127, 0, 0, 1], lp).into();
    let t0 = std::time::Instant::now();
    let prog = run_program(&dir, lp, wp);
    let started = t0.elapsed();
    let (mut w, lines) = into_world_at(login_addr);
    let in_world = t0.elapsed();
    eprintln!("smoke: started in {started:?}, in the world at {in_world:?}");
    for l in &lines {
        eprintln!("smoke: login line: {l}");
    }
    assert!(lines[0].starts_with("Welcome to your Den: 0 props"), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains(".den place chair")), "{lines:?}");

    // the quick start's own commands, as typed
    let say = |w: &mut World, cmd: &str| -> String {
        w.say(cmd);
        let l = w.until_line().1;
        eprintln!("smoke: {cmd} -> {l}");
        l
    };
    assert!(say(&mut w, ".den find lamp").contains(".den place"), "the quick start's .den find lamp finds something");
    assert!(say(&mut w, ".den place chair").contains("is down (1 props)"));
    assert!(say(&mut w, ".den stamp inn now").contains("is down"));
    assert!(say(&mut w, ".build undo").len() > 0);
    w.send(&heartbeat(16226.2 - 3.0, 16257.0, 14.2, 0.0));
    assert!(say(&mut w, ".den place lantern").contains("is down (1 props)"));
    let saved = say(&mut w, ".den save mine");
    assert!(saved.starts_with("Saved to a file (2 props)"), "{saved}");
    assert_eq!(den::read_file(&dir.join("mine.den.json")).unwrap().props.len(), 2);

    // the program stops (the launcher's stop: a kill) and starts again on the same folder
    // the launcher knows it by its heartbeat and stops it with the stop file (host/lite.rs)
    let beat = std::fs::read_to_string(dir.join("den-lite.alive")).expect("the heartbeat");
    assert_eq!(beat.split_whitespace().nth(2), Some(lp.to_string().as_str()), "{beat}");
    drop(w);
    let mut prog = prog;
    std::fs::write(dir.join("den-lite.stop"), "").unwrap();
    let mut stopped = None;
    for _ in 0..60 {
        stopped = prog.0.try_wait().unwrap();
        if stopped.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(stopped.is_some_and(|s| s.success()), "the stop file stops it");
    assert!(!dir.join("den-lite.alive").exists() && !dir.join("den-lite.stop").exists());
    let t1 = std::time::Instant::now();
    let _prog = run_program(&dir, lp, wp);
    let (mut w, lines) = into_world_at(login_addr);
    eprintln!("smoke: restarted and back in the world in {:?}", t1.elapsed());
    eprintln!("smoke: after the restart: {}", lines[0]);
    assert!(say(&mut w, ".den files").contains("mine"));
    let opened = say(&mut w, ".den load mine");
    assert!(opened.starts_with("Opened: 2 props") || opened.contains("replace"), "{opened}");
    if opened.contains("replace") {
        assert_eq!(say(&mut w, ".den load mine replace"), "Opened: 2 props.");
    }
    assert_eq!(say(&mut w, ".den size").split(':').next().unwrap(), "Your Den is size 1 of 7");
    assert!(say(&mut w, ".den grow").starts_with("Your Den is size 2 of 7"));
    // the real program: she is at your side from login, so calling her answers in her own voice
    w.say(".den companion");
    assert_eq!(w.until_she_says().2, den_lite::companion::HERE);
    assert!(say(&mut w, ".den mode").starts_with("Host templates"));
    w.until_line();
    w.until_line();
    assert!(say(&mut w, ".den mods").starts_with("Your game loaded 2 addons"));
    w.until_line();
    w.until_line();
    assert!(say(&mut w, ".den").starts_with(".den place"));
    w.send(&CMSG_LOGOUT_REQUEST {});
    w.until(|f| matches!(f.msg, Some(ServerOpcodeMessage::SMSG_LOGOUT_COMPLETE)).then_some(()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The solo session's helper: she is there from login (one greeting), takes "Jaina, stay / come / follow / tips"
/// without writing them down for the assistant, answers from her own guide with no assistant connected, stays put
/// when told and walks over when called, and never comes back once sent away.
#[test]
fn the_companion_is_there_from_login_and_takes_short_commands() {
    let (srv, dir) = server_with("helper", HeroGender::Female, true);
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    let (mx, my, mz) = (16226.2f32, 16257.0f32, 14.2f32);
    // her create and one greeting, after the quick start
    let created = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => u.objects.iter().any(|o| matches!(o, Object::CreateObject { guid3, .. } if guid3.guid() == den_lite::companion::GUID)).then_some(()),
        _ => None,
    });
    let _ = created;
    assert_eq!(w.until_she_says().2, den_lite::companion::GREET);
    assert!(srv.shared.companion.lock().unwrap().out);
    // tips that fit: the Den is empty, so the first one places a chair (no assistant needed)
    w.say("Jaina, tips");
    assert!(w.until_she_says().2.contains(".den place chair"));
    // a question her guide knows, with nobody reading her notes: her own answer, and it is still written down
    w.say("Jaina, how do I save my den?");
    let a = w.until_she_says().2;
    assert!(a.starts_with(".den save"), "{a}");
    std::thread::sleep(Duration::from_millis(600));
    let cin: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("companion-in.json")).unwrap()).unwrap();
    assert_eq!(cin["asks"].as_array().unwrap().len(), 1, "the command (tips) isn't an ask; the question is");
    // stay: the player walks 12 yards, she doesn't follow
    w.say("Jaina, stay");
    assert_eq!(w.until_she_says().2, den_lite::companion::STAYING);
    let id0 = srv.shared.companion.lock().unwrap().moves;
    w.send(&heartbeat(mx + 12.0, my, mz, 0.0));
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(srv.shared.companion.lock().unwrap().moves, id0, "staying");
    // come, from 12 yards (beyond hearing for asks, but commands reach her anywhere in the Den): she walks over
    w.say("Jaina, come here");
    let mv = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MONSTER_MOVE(m)) => Some(m.clone()),
        _ => None,
    });
    let (sx, sy) = (mx + 12.0 + (0.75 * std::f32::consts::PI).cos() * 2.5, my + (0.75 * std::f32::consts::PI).sin() * 2.5);
    assert!((mv.splines[0].x - sx).abs() < 0.01 && (mv.splines[0].y - sy).abs() < 0.01, "{:?}", mv.splines[0]);
    assert_eq!(w.until_she_says().2, den_lite::companion::COMING);
    assert_eq!(srv.shared.companion.lock().unwrap().order, den_lite::companion::Order::Follow);
    // sent away: gone, and walking out and back in doesn't bring her
    w.say("Jaina, go away");
    assert_eq!(w.until_she_says().2, den_lite::companion::BYE);
    w.send(&heartbeat(mx + 40.0, my, mz, 0.0));
    w.send(&heartbeat(mx, my, mz, 0.0));
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!srv.shared.companion.lock().unwrap().out);
    w.say(".den companion tips");
    assert!(w.until_line().1.contains(".den place chair"), "tips without her: a system line");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Smoke O1: the Den keeps itself. A piece placed is in the autosave within seconds, and a Den Lite started again on
/// the same folder (the PC restarted) opens it: the player walks into their room as they left it, nothing to load,
/// and undo has nothing to take back.
#[test]
fn the_den_is_kept_by_itself_and_open_again_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("den-lite-test-autosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = || Config { login_port: 0, world_port: 0, den_dir: dir.clone(), stamps_dir: stamps(), companion: false, ui_load_ms: 300, ..Config::default() };
    let srv = start(cfg()).unwrap();
    assert!(!dir.join("autosave.den.json").exists(), "an empty new Den writes nothing");
    let mut w = into_world(&srv, 0);
    w.say(".den place wooden chair");
    assert!(w.until_line().1.starts_with("Wooden Chair is down"));
    w.say(".den room stone");
    w.until_line();
    std::thread::sleep(Duration::from_millis(2600));
    let kept = den::read_file(&dir.join("autosave.den.json")).expect("the autosave");
    assert_eq!((kept.props.len(), kept.room.as_ref().unwrap().look.as_str()), (1, "stone"));
    let at = std::fs::metadata(dir.join("autosave.den.json")).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(2200));
    assert_eq!(std::fs::metadata(dir.join("autosave.den.json")).unwrap().modified().unwrap(), at, "unchanged: not written again");
    drop(w);
    // started again on the same folder
    let srv2 = start(cfg()).unwrap();
    let mut w = into_world(&srv2, 0);
    let d = srv2.shared.den.lock().unwrap();
    assert_eq!((d.props.len(), d.look.name), (1, "stone"));
    drop(d);
    w.say(".build undo");
    assert_eq!(w.until_line().1, "Nothing to undo.", "nothing to undo after a restart");
    assert_eq!(srv2.shared.den.lock().unwrap().props.len(), 1);
    // the tips don't count the autosave as a save of the player's own
    w.say(".den companion tips");
    assert!(w.until_line().1.contains(".den save"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Into the world on a given world address with a session key; returns the client and its character's guid.
fn enter(address: &str, account: &str, key: [u8; 40]) -> (World, u64) {
    let mut w = World::connect(address, account, key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let (guid, name) = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some((e.characters[0].guid, e.characters[0].name.clone())),
        _ => None,
    });
    let _ = name;
    w.send(&CMSG_PLAYER_LOGIN { guid });
    (w, guid.guid())
}

/// A player create for `guid` in the frames (Some(true): with the self flag).
fn player_created(frames: &[Frame], guid: u64) -> Option<bool> {
    frames.iter().find_map(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => u.objects.iter().find_map(|o| match o {
            Object::CreateObject2 { guid3, movement2, object_type: ObjectType::Player, .. } if guid3.guid() == guid => {
                Some(movement2.update_flag.get_self())
            }
            _ => None,
        }),
        _ => None,
    })
}

/// Friends (guests.rs, the remote listeners): a one-time guest login made for an invite gets in through the friends'
/// listener and is sent to the tunnel's address; the host's fixed login is refused there; both players see each
/// other, their moves and what they say, and the props; only the host builds until they allow it; logging out
/// takes the friend away for the host; a login taken back no longer works.
#[test]
fn a_friend_joins_through_the_friends_listener_and_both_see_each_other() {
    let (srv, dir) = server("friends", HeroGender::Female);
    let (rl, rw) = srv.remote.expect("the friends' listeners");
    assert_eq!(std::fs::read_to_string(dir.join(den_lite::REMOTE_FILE)).unwrap().trim(), format!("{} {}", rl.port(), rw.port()));
    let g = den_lite::guests::add(&dir, Some("rowan"), "Wren", 24, now()).unwrap();
    // the host's own login: fine here, never through the friends' listener
    assert!(login(rl, "DEN", "den").is_err(), "the fixed local login is refused from outside");
    assert!(login(rl, &g.user, "WRONGPASSWORD123").is_err());
    // the tunnel's address goes in the friend's realm list
    std::fs::write(dir.join("den-lite.public"), "relay.example.net:40001\n").unwrap();
    let (gkey, _, gaddr) = login(rl, &g.user, &g.pass).expect("the guest's own login");
    assert_eq!(gaddr, "relay.example.net:40001", "friends go back through the relay");
    let (_, _, haddr) = login(srv.login, "DEN", "den").unwrap();
    assert_eq!(haddr, srv.world.to_string(), "the host plays on this PC");
    // the host in the world, with a chair down
    let mut host = into_world(&srv, 0);
    host.say(".den place wooden chair");
    assert!(host.until_line().1.starts_with("Wooden Chair is down"));
    // the guest's login can't use the host's world listener as the host (and the host's key isn't valid remotely)
    let (hkey, _, _) = login(srv.login, "DEN", "den").unwrap();
    let mut bad = World::connect(&rw.to_string(), "DEN", hkey, 0);
    // refused: one short unencrypted answer (AUTH_UNKNOWN_ACCOUNT, 0x15), then the server hangs up
    let mut rest = Vec::new();
    let _ = bad.s.read_to_end(&mut rest);
    assert!(rest.len() <= 8 && rest.last() == Some(&0x15), "the host's session can't come in on the friends' world listener: {rest:?}");
    // the guest comes in through the friends' world listener (the tunnel connects there)
    let (mut friend, fguid) = enter(&rw.to_string(), &g.user, gkey);
    assert_eq!(fguid, den_lite::guests::GUID_BASE);
    let (frames, line) = friend.until_line();
    assert!(line.starts_with("Welcome to Wren's Den"), "{line}");
    let (props, _) = objects(&frames);
    assert!(!props.is_empty(), "the host's chair");
    let (frames, _) = friend.until_line();
    let _ = frames;
    // the host sees the friend arrive (no self flag), and the friend sees the host
    let (frames, line) = host.until_line();
    assert_eq!(line, "Rowan is in your Den.");
    assert_eq!(player_created(&frames, fguid), Some(false));
    friend.send(&CMSG_NAME_QUERY { guid: Guid::new(den_lite::world::PLAYER_GUID) });
    let n = friend.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_NAME_QUERY_RESPONSE(r)) => Some(r.character_name.clone()),
        _ => None,
    });
    assert_eq!(n, "Wren");
    // the host moves: the friend gets the move with the host's guid
    host.send(&heartbeat(16226.2 + 3.0, 16257.0, 14.2, 0.0));
    let mv = friend.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::MSG_MOVE_HEARTBEAT(m)) => Some(m.clone()),
        _ => None,
    });
    assert_eq!((mv.guid.guid(), mv.info.position.x), (den_lite::world::PLAYER_GUID, 16226.2 + 3.0));
    // what the friend says, the host hears from the friend
    friend.say("hello there");
    let heard = host.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::Say { .. }) => Some((m.sender.guid(), m.message.clone())),
        _ => None,
    });
    assert_eq!(heard, (fguid, "hello there".to_string()));
    // only the host builds; the host's own tools are theirs
    friend.say(".den place lantern");
    assert!(friend.until_line().1.starts_with("Your host builds here"));
    friend.say(".den save mine");
    assert!(friend.until_line().1.starts_with("As a guest you can"));
    host.say(".den friends build on");
    assert_eq!(host.until_line().1, "Your friends can build in your Den now.");
    assert_eq!(friend.until_line().1, "Your friends can build in your Den now.");
    friend.say(".den place lantern");
    assert!(friend.until_line().1.contains("is down"));
    // the host sees the friend's lantern
    host.until(|f| (f.opcode == 0x00A9).then_some(()));
    host.say(".den friends");
    assert!(host.until_line().1.starts_with("Friends here: Rowan. Invites that still work: 1."));
    // the friend logs out: gone for the host
    friend.send(&CMSG_LOGOUT_REQUEST {});
    let gone = host.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) if d.guid.guid() == fguid => Some(()),
        _ => None,
    });
    let _ = gone;
    assert_eq!(srv.shared.players.lock().unwrap().len(), 1);
    // taken back: the login no longer works
    den_lite::guests::revoke(&dir, &g.user, now()).unwrap();
    assert!(login(rl, &g.user, &g.pass).is_err());
    // the log says the host's game was shown the friend (den-test --invite reads this)
    let ev = den_lite::events::read(&dir);
    assert!(ev.iter().any(|e| e.ev == "peer_seen" && e.who == "Wren" && e.detail == g.name), "{ev:?}");
    assert!(ev.iter().any(|e| e.ev == "auth_ok" && e.who == g.user && e.detail == "remote"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `den-lite guest add` (what the launcher runs): one line of JSON with the login, once; nothing secret on disk.
#[test]
fn the_guest_command_prints_a_login_once() {
    let dir = std::env::temp_dir().join(format!("den-lite-test-guestcmd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_den-lite"))
        .args(["guest", "add", "--dens", &dir.display().to_string(), "--name", "Fen", "--hours", "2"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let pass = v["pass"].as_str().unwrap();
    assert_eq!((v["name"].as_str(), pass.len()), (Some("Fen"), 16));
    assert!(!std::fs::read_to_string(dir.join("guests.json")).unwrap().contains(pass));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_den-lite")).args(["guest", "list", "--dens", &dir.display().to_string()]).output().unwrap();
    let l: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(l[0]["name"], "Fen");
    assert!(l[0].get("pass").is_none() && l[0].get("verifier").is_none());
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_den-lite")).args(["guest", "revoke", "--dens", &dir.display().to_string(), "all"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "{\"revoked\":1}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The opcodes the real game sends after the one-click sign-in (the launcher's keys or our login screen code):
/// the realm list, character list, a character made (Den Lite keeps its ready-made one), enter the world; then what
/// the Leyline UI sends in the world: the Den panel's hello, the Builder's catalog, spawn, move, turn, delete, undo,
/// "Invite a friend" asked of the launcher and its answer back in the panel. Every step is in den-lite.events.
#[test]
fn one_click_lands_in_the_den_with_the_builder_ready() {
    let (srv, dir) = server("oneclick", HeroGender::Female);
    let ev = |dir: &std::path::Path| den_lite::events::read(dir).into_iter().map(|l| l.ev).collect::<Vec<_>>();
    // at start: "start" (and "models", how many prop models it knows, when the game's table is there)
    assert_eq!(ev(&dir).into_iter().filter(|e| e != "models" && e != "my_models").collect::<Vec<_>>(), ["start"]);
    // the login screen code signs in as DENAUTO (the launcher's keys: DEN, as Config.wtf fills in)
    let (key, _, address) = login(srv.login, "DENAUTO", "den").expect("login");
    let mut w = World::connect(&address, "DENAUTO", key, 3);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters.clone()),
        _ => None,
    });
    assert_eq!(ch.len(), 1, "one ready-made character, for any account name");
    w.send(&CMSG_CHAR_CREATE { name: "Wren".into(), race: Race::Human, class: Class::Warrior, gender: Gender::Female, skin_color: 0, face: 0, hair_style: 0, hair_color: 0, facial_hair: 0 });
    let made = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_CREATE(c)) => Some(c.result),
        _ => None,
    });
    assert_eq!(made, WorldResult::CharCreateSuccess, "a character made by the login screen code doesn't hang it");
    w.send(&CMSG_PLAYER_LOGIN { guid: ch[0].guid });
    let (_, welcome) = w.until_line();
    assert!(welcome.contains("F7 or /builder") && welcome.contains("/den (Invite a friend, Shape the ground"), "{welcome}");
    for _ in 0..5 {
        w.until_line();
    }

    // the Den panel's hello (EditMode.lua E.Hello at PLAYER_ENTERING_WORLD)
    w.addon("OVMOD", "wuden\tinfo\t1/1\t");
    assert_eq!(w.until_addon("wuden", "info"), "role=owner;theme=white;builders=-;invite=1");
    // the Builder: its catalog in parts, then spawn a chair, move it up, turn it, delete it and undo
    w.addon("OVMOD", "wubuild\tcatalog\t1/1\t");
    let cat = w.until_addon("wubuild", "catalog");
    assert!(cat.matches(';').count() > 50 && cat.contains("2413,woodenchair,seating,Wooden Chair,1;"));
    w.send(&heartbeat(16226.2, 16257.0, 14.2, 0.0));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tspawn 2413");
    // the state after the spawn (the catalog may be followed by a state of its own, before anything is placed)
    let mut st = w.until_addon("wubuild", "state");
    for _ in 0..4 {
        if st.contains("n=1;") {
            break;
        }
        st = w.until_addon("wubuild", "state");
    }
    let id: u64 = st.split("sel=").nth(1).unwrap().split(',').next().unwrap().parse().expect(&st);
    assert!(st.contains(",2413,Wooden Chair;") && st.contains("n=1;") && st.contains("den=owner"), "{st}");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tmove z 0.5");
    let st = w.until_addon("wubuild", "state");
    assert!(st.contains(",13.70,"), "half a yard up from the floor: {st}");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tturn 45");
    assert!(w.until_addon("wubuild", "state").contains("rot=45,0,0"));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tdelete");
    let gone = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) => Some(d.guid.guid()),
        _ => None,
    });
    assert_eq!(gone & 0xFF_FFFF, id);
    assert!(w.until_addon("wubuild", "state").contains("sel=-;"));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tundo");
    assert!(w.until_addon("wubuild", "state").contains("mine=1/"));
    // the Den keeps itself: the chair is in the autosave within a few seconds
    let auto = dir.join("autosave.den.json");
    let mut kept = false;
    for _ in 0..40 {
        if std::fs::read_to_string(&auto).is_ok_and(|t| t.contains("\"entry\": 2413") || t.contains("\"entry\":2413")) {
            kept = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    assert!(kept, "the Builder's chair is autosaved");

    // Invite a friend: asked of the launcher, its answer lands in the Den panel
    w.addon("OVMOD", "wuden\tcmd\t1/1\tinvite");
    assert!(w.until_line().1.starts_with("Making an invite link"));
    let ask = std::fs::read_to_string(dir.join(den_lite::ovmod::ASK_FILE)).unwrap();
    assert!(ask.contains("\"ask\":\"invite\""), "{ask}");
    let link = "https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789";
    std::fs::write(dir.join(den_lite::ovmod::ANSWER_FILE), format!("{{\"id\":1,\"invite\":\"{link}\"}}")).unwrap();
    assert_eq!(w.until_addon("wuden", "invite"), link);
    // no Den bar's hello in this game (an older addon): the line in the chat, after the link
    assert!(w.until_line().1.starts_with("Your invite link is in the Den panel"));
    // Shape the ground: asked too
    w.say(".den forge");
    assert!(w.until_line().1.starts_with("Opening Forge"));
    assert!(std::fs::read_to_string(dir.join(den_lite::ovmod::ASK_FILE)).unwrap().contains("forge"));

    let got = ev(&dir);
    for want in ["start", "login_challenge", "auth_ok", "realm_list", "world_auth", "char_enum", "char_create", "player_login", "in_world", "builder_ready", "first_move", "ask"] {
        assert!(got.iter().any(|e| e == want), "{want} missing from {got:?}");
    }
    let order: Vec<&str> = got.iter().map(String::as_str).filter(|e| ["auth_ok", "char_enum", "player_login", "in_world"].contains(e)).collect();
    assert_eq!(order, ["auth_ok", "char_enum", "player_login", "in_world"]);
    let lines = den_lite::events::read(&dir);
    assert!(lines.iter().any(|l| l.ev == "login_challenge" && l.who == "DENAUTO"), "which way it signed in shows");
    let text = std::fs::read_to_string(dir.join(den_lite::events::FILE)).unwrap();
    assert!(!text.contains("\"den\""), "the password is never written");
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The den-test's commands through the Dens folder (testctl.rs): what each said and did comes back in
/// den-lite.test-out, the events show the build, the save, the load and the autosave; off unless asked for.
#[test]
fn the_den_test_types_through_the_dens_folder_not_the_keyboard() {
    use den_lite::testctl::{CMD_FILE, OUT_FILE};
    let dir = std::env::temp_dir().join(format!("den-lite-test-testctl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config { login_port: 0, world_port: 0, den_dir: dir.clone(), stamps_dir: stamps(), companion: false, test_control: true, ui_wait_ms: 300, ui_load_ms: 300, ..Config::default() };
    let srv = start(cfg).expect("den lite starts");
    let mut w = into_world(&srv, 2);
    w.send(&heartbeat(16226.2, 16257.0, 14.2, 0.0));
    let send = |id: u64, cmd: &str| -> serde_json::Value {
        let tmp = dir.join("cmd.tmp");
        std::fs::write(&tmp, serde_json::json!({"id": id, "cmd": cmd}).to_string()).unwrap();
        std::fs::rename(&tmp, dir.join(CMD_FILE)).unwrap();
        for _ in 0..80 {
            let out = std::fs::read_to_string(dir.join(OUT_FILE)).unwrap_or_default();
            if let Some(v) = out.lines().filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok()).find(|v| v["id"] == id) {
                return v;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("no answer to {cmd}");
    };
    let size = send(1, ".den size");
    assert_eq!((size["props"].as_u64(), size["look"].as_str(), size["size"].as_u64()), (Some(0), Some("white"), Some(1)), "{size}");
    assert!(size["said"][0].as_str().unwrap().contains("16"), "{size}");
    let placed = send(2, ".den place chair");
    assert!(placed["created"].as_u64().unwrap() >= 1 && placed["props"] == 1, "{placed}");
    let undone = send(3, ".build undo");
    assert!(undone["destroyed"].as_u64().unwrap() >= 1 && undone["props"] == 0, "{undone}");
    send(4, ".den place lantern");
    let saved = send(5, ".den save smoketest");
    assert!(saved["said"][0].as_str().unwrap().starts_with("Saved to a file (1 props)"), "{saved}");
    let text = std::fs::read_to_string(dir.join("smoketest.den.json")).unwrap();
    assert!(text.contains("\"leyline-den\""));
    let loaded = send(6, ".den load smoketest replace");
    assert_eq!(loaded["props"], 1, "{loaded}");
    assert!(dir.join("backup.den.json").is_file(), "a replacing load keeps a backup");
    let room = send(7, ".den room stone");
    assert!(room["said"].as_array().unwrap().iter().any(|l| l == "The room is stone now.") && room["look"] == "stone", "{room}");
    send(8, ".den room white");
    // only Den commands: anything else is refused and answered at once
    let refused = send(9, "hello everyone");
    assert_eq!(refused["refused"], true);
    // the autosave follows within a few seconds
    let mut saw = false;
    for _ in 0..50 {
        if den_lite::events::read(&dir).iter().any(|l| l.ev == "autosave") {
            saw = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(saw, "an autosave event");
    let evs = den_lite::events::read(&dir);
    for want in ["test_cmd", "den_change", "den_save", "den_load"] {
        assert!(evs.iter().any(|l| l.ev == want), "{want}");
    }
    assert!(evs.iter().any(|l| l.ev == "den_load" && l.detail == "smoketest replace"));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);

    // without test control the file is never taken
    let (srv, dir) = server("testctl-off", HeroGender::Female);
    let _w = into_world(&srv, 0);
    std::fs::write(dir.join(CMD_FILE), r#"{"id":1,"cmd":".den place chair"}"#).unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    assert!(dir.join(CMD_FILE).exists() && !dir.join(OUT_FILE).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Frames until the addon answer `wuden <name>` (one part); returns the frames before it and its data.
fn until_den(w: &mut World, name: &str) -> (Vec<Frame>, String) {
    let want = format!("OVMOD\twuden\t{name}\t1/1\t");
    let mut got = Vec::new();
    loop {
        let f = w.frame();
        if let Some(d) = addon_text(&f).and_then(|t| t.strip_prefix(&want).map(String::from)) {
            return (got, d);
        }
        got.push(f);
    }
}

/// The system lines and her lines among frames (what the chat frame would show).
fn chat_lines(frames: &[Frame]) -> Vec<String> {
    frames
        .iter()
        .filter_map(|f| match &f.msg {
            Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m))
                if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::System { .. } | SMSG_MESSAGECHAT_ChatType::MonsterSay { .. }) =>
            {
                Some(m.message.clone())
            }
            _ => None,
        })
        .collect()
}

/// The Den bar (WrathUnleashedUI DenBar.lua; notice.rs; owner 1 Nov 23:46: "make it make sense, not spam the chat"):
/// once the bar says hello, what Den Lite says that nobody typed for is a toast in the bar: the welcome, Jaina's one
/// greeting, the Builder's answers, the invite link being made and ready, the launcher's answer, a save from the
/// bar, the autosave; never a chat line. What the player types still answers in the chat.
#[test]
fn with_the_den_bar_nothing_spams_the_chat() {
    let dir = std::env::temp_dir().join(format!("den-lite-test-bar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config { login_port: 0, world_port: 0, den_dir: dir.clone(), stamps_dir: stamps(), companion: true, ui_wait_ms: 8000, ui_load_ms: 8000, cheer_gap: 0, ..Config::default() };
    let srv = start(cfg).expect("den lite starts");
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 3);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    // the bar opens and says hello (DenBar.lua at PLAYER_ENTERING_WORLD): its status, then what waited for it
    w.addon("OVMOD", "wuden\thello\t1/1\t1");
    let (frames, status) = until_den(&mut w, "status");
    assert_eq!(status, "role=owner;props=0;saved=-");
    assert!(chat_lines(&frames).is_empty(), "nothing in the chat on the way in: {:?}", chat_lines(&frames));
    let (frames, welcome) = until_den(&mut w, "toast");
    assert_eq!(welcome, format!("cheer|{}", den_lite::cheers::LINES[0].1), "the first visit's one line");
    let (more, greet) = until_den(&mut w, "toast");
    assert_eq!(greet, format!("jaina|{}", den_lite::companion::GREET), "her one greeting, as a toast");
    assert!(chat_lines(&frames).is_empty() && chat_lines(&more).is_empty() && w.notes.is_empty(), "{:?}", w.notes);

    // the den-test's before / after: Den Lite asks the bar for the game's view of the character, and says so
    w.say(".den diag before");
    let (_, tag) = until_den(&mut w, "diag");
    assert_eq!(tag, "before");
    let (_, line) = w.until_line();
    assert!(line.starts_with("Diag before: asked the Leyline UI; your game sent: "), "{line}");

    // the Builder (a bar's answer): the change for everyone, its line a toast
    w.send(&heartbeat(16226.2, 16257.0, 14.2, 0.0));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tspawn 2413");
    let (frames, placed) = until_den(&mut w, "toast");
    assert!(placed.starts_with("info|Placed."), "{placed}");
    let (made, _) = objects(&frames);
    assert_eq!(made.len(), 1, "the chair is there");
    // the autosave tells the bar ("Saved 2 s ago"), nothing in the chat
    let (frames, auto) = until_den(&mut w, "saved");
    assert_eq!(auto, "auto");
    assert!(chat_lines(&frames).is_empty(), "{:?}", chat_lines(&frames));
    w.addon("OVMOD", "wuden\tstatus\t1/1\t");
    let (_, status) = until_den(&mut w, "status");
    assert!(status.starts_with("role=owner;props=1;saved="), "{status}");

    // the bar's Save: the line a toast, and the bar hears the name
    w.addon("OVMOD", "wuden\tcmd\t1/1\tsave mine");
    let (frames, name) = until_den(&mut w, "saved");
    assert_eq!(name, "mine");
    let (more, a) = until_den(&mut w, "toast");
    let (more2, b) = until_den(&mut w, "toast");
    let mut two = [a, b];
    two.sort();
    assert_eq!(two[0], format!("cheer|{}", den_lite::cheers::LINES[2].1), "the first save's line");
    assert!(two[1].starts_with("info|Saved to a file (1 props)"), "{two:?}");
    assert!(chat_lines(&frames).is_empty() && chat_lines(&more).is_empty() && chat_lines(&more2).is_empty());

    // Invite (the bar's button): being made, then the link and its toast
    w.addon("OVMOD", "wuden\tcmd\t1/1\tinvite");
    let (frames, making) = until_den(&mut w, "toast");
    assert!(making.starts_with("info|Making an invite link"), "{making}");
    assert!(chat_lines(&frames).is_empty());
    let link = "https://relay.openvanilla.app/join/kzdvvj2umnduyauf#ABCDEFGHJKLMNPQRSTUVWXYZ~~GABCDE2345.ABCDEFGH23456789";
    std::fs::write(dir.join(den_lite::ovmod::ANSWER_FILE), format!("{{\"id\":1,\"invite\":\"{link}\"}}")).unwrap();
    let (_, got) = until_den(&mut w, "invite");
    assert_eq!(got, link);
    let (frames, ready) = until_den(&mut w, "toast");
    assert!(ready.starts_with("ok|Your invite link is ready"), "{ready}");
    assert!(chat_lines(&frames).is_empty());

    // Shape ground (the bar's button) and the launcher's answer: toasts
    w.addon("OVMOD", "wuden\tcmd\t1/1\tforge");
    assert!(until_den(&mut w, "toast").1.starts_with("info|Opening Forge"));
    std::fs::write(dir.join(den_lite::ovmod::ANSWER_FILE), r#"{"id":2,"say":"Forge is open on your Den's tile."}"#).unwrap();
    let (frames, said) = until_den(&mut w, "toast");
    assert_eq!(said, "info|Forge is open on your Den's tile.");
    assert!(chat_lines(&frames).is_empty());

    // typed: the chat, as always
    w.say(".den forge");
    assert!(w.until_line().1.starts_with("Opening Forge"));
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "den_ui"), "the bar's hello is in the events");
    assert!(evs.iter().any(|l| l.ev == "notice" && l.detail == "toast cheer"));
    assert!(!evs.iter().any(|l| l.ev == "notice" && l.detail.starts_with("chat")), "nothing the old way");
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A game without the bar (or an older one): after the wait, the old way: the welcome and the quick start in the
/// chat, the first visit's line in the middle of the screen, her greeting as her line.
#[test]
fn without_the_den_bar_the_old_lines_after_a_short_wait() {
    let (srv, dir) = server_with("nobar", HeroGender::Female, true);
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    let t = std::time::Instant::now();
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    let (_, welcome) = w.until_line();
    assert!(t.elapsed() >= Duration::from_millis(300), "it waited for a hello first");
    assert!(welcome.starts_with("Welcome to your Den: 0 props"), "{welcome}");
    let quick: Vec<String> = (0..5).map(|_| w.until_line().1).collect();
    assert!(quick[0].starts_with("Quick start: 1) .den place chair"), "{quick:?}");
    assert_eq!(w.until_she_says().2, den_lite::companion::GREET);
    assert_eq!(w.notes, vec![den_lite::cheers::LINES[0].1.to_string()]);
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "notice" && l.detail == "chat cheer") && !evs.iter().any(|l| l.ev == "den_ui"));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);

    // a long loading screen doesn't count: the wait starts when the game is in the world (its first move here)
    let dir = std::env::temp_dir().join(format!("den-lite-test-loading-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config { login_port: 0, world_port: 0, den_dir: dir.clone(), stamps_dir: stamps(), companion: false, ui_wait_ms: 300, ui_load_ms: 60_000, ..Config::default() };
    let srv = start(cfg).expect("den lite starts");
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    std::thread::sleep(Duration::from_millis(1200));
    assert!(!den_lite::events::read(&dir).iter().any(|l| l.ev == "notice"), "still loading: nothing said yet");
    w.send(&heartbeat(16226.2, 16257.0, 14.2, 0.0));
    let t = std::time::Instant::now();
    assert!(w.until_line().1.starts_with("Welcome to your Den"));
    assert!(t.elapsed() < Duration::from_secs(3), "soon after the game is in the world");
    assert!(den_lite::events::read(&dir).iter().any(|l| l.ev == "loaded"));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Dojo (dojo.rs, owner 3 Nov): flying and speed with the game's acks, "Jaina, build a garden" piece by piece
/// with her casts, the training ring with its dummies, what needs the full Den, the dummies down and the ring
/// undone, and the Den bar's Dojo line.
#[test]
fn the_dojo_flies_speeds_and_jaina_builds_piece_by_piece() {
    let (srv, dir) = server_with("dojo", HeroGender::Female, true);
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    assert_eq!(w.until_she_says().2, den_lite::companion::GREET);
    let (mx, my, mz) = (16226.2f32, 16257.0f32, 13.25f32);
    w.send(&heartbeat(mx, my, mz, 0.0));

    // fly: the game is told it may (the player's packed guid, a counter); its ack is read: counted, and where it is
    w.say(".den fly on");
    let fly = w.until(|f| (f.opcode == 0x343).then(|| f.body.clone()));
    assert_eq!(&fly[..2], &[0x01, 0x01], "the player's packed guid");
    let n = u32::from_le_bytes(fly[2..6].try_into().unwrap());
    assert!(w.until_line().1.starts_with("You can fly"));
    let mut ack = fly.clone();
    ack.extend_from_slice(&0x0100_0000u32.to_le_bytes());
    ack.extend_from_slice(&0u16.to_le_bytes());
    ack.extend_from_slice(&5000u32.to_le_bytes());
    for v in [mx, my, mz + 3.0, 0.0f32] {
        ack.extend_from_slice(&v.to_le_bytes());
    }
    ack.extend_from_slice(&0f32.to_le_bytes());
    w.raw(0x345, &ack);
    // speed: run and flight, three times the normal 7 yards a second
    w.say(".den speed 3");
    let run = w.until(|f| (f.opcode == 0x0E2).then(|| f.body.clone()));
    assert_eq!(f32::from_le_bytes(run[7..11].try_into().unwrap()), 21.0);
    let flight = w.until(|f| (f.opcode == 0x381).then(|| f.body.clone()));
    assert_eq!(f32::from_le_bytes(flight[6..10].try_into().unwrap()), 21.0);
    assert!(w.until_line().1.contains("3 times as fast"));
    {
        let d = srv.shared.dojo.lock().unwrap();
        assert_eq!(d.acks.0, 1, "the fly ack was read");
        assert!(d.fly && d.speed_now() == 3);
    }
    let live = srv.shared.live.lock().unwrap().as_ref().map(|l| l.2).unwrap();
    assert!((live.z - (mz + 3.0)).abs() < 1e-4, "the ack says where the character is");
    w.say(".den speed 9");
    assert!(w.until_line().1.starts_with("Speeds are 1 to 5"));
    w.say(".den fly off");
    let off = w.until(|f| (f.opcode == 0x344).then(|| f.body.clone()));
    assert_eq!(u32::from_le_bytes(off[2..6].try_into().unwrap()), n + 3, "a new counter each time");
    assert!(w.until_line().1.starts_with("Flying is off"));

    // "Jaina, build a garden": she walks over and says so; the pieces are in the Den at once (one undo), and the
    // game is shown them one by one, each after her turn to it and her cast at its spot
    w.send(&heartbeat(mx, my, mz, 0.0));
    let t0 = std::time::Instant::now();
    w.say("Jaina, build a garden");
    let mv = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_MONSTER_MOVE(m)) => Some(m.clone()),
        _ => None,
    });
    assert_eq!(mv.guid.guid(), den_lite::companion::GUID);
    assert_eq!(w.until_she_says().2, den_lite::dojo::ON_IT);
    let placed = srv.shared.den.lock().unwrap().props.len();
    assert!(placed > 3, "the garden is in the Den: {placed}");
    let mut frames = Vec::new();
    loop {
        let f = w.frame();
        if let Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) = &f.msg {
            if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::MonsterSay { .. }) {
                assert_eq!(m.message, den_lite::dojo::BUILT);
                break;
            }
        }
        frames.push(f);
    }
    let took = t0.elapsed();
    let (made, _) = objects(&frames);
    assert_eq!(made.len(), placed, "every piece appeared");
    let starts = frames.iter().filter(|f| f.opcode == 0x131).count();
    let gos: Vec<&Frame> = frames.iter().filter(|f| f.opcode == 0x132).collect();
    let turns: Vec<&Frame> = frames.iter().filter(|f| f.opcode == 0x0DD && f.msg.is_none()).collect();
    assert_eq!((starts, gos.len(), turns.len()), (1, placed, placed + 1), "one cast started, a turn and a cast per piece, then her step back");
    // the last: she walks back (longer than a turn on the spot) and ends facing the garden (a facing-angle move)
    let back = &turns[placed].body;
    let n = 1 + back[0].count_ones() as usize;
    assert_eq!(back[n + 17], 4, "a facing-angle move");
    assert!(u32::from_le_bytes(back[n + 26..n + 30].try_into().unwrap()) > 100, "a walk, not a turn on the spot");
    assert!(took >= Duration::from_millis(den_lite::dojo::PIECE_MS * (placed as u64 - 1)), "one at a time: {took:?}");
    let first_go = frames.iter().position(|f| f.opcode == 0x132).unwrap();
    let first_piece = frames.iter().position(|f| matches!(&f.msg, Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(_)))).unwrap();
    assert!(first_go < first_piece, "her cast, then the piece");
    // her cast is aimed at a piece's spot (the go's place target: its last 13 bytes are x, y, z and a 0)
    let g = &gos[0].body;
    let gx = f32::from_le_bytes(g[g.len() - 13..g.len() - 9].try_into().unwrap());
    let gy = f32::from_le_bytes(g[g.len() - 9..g.len() - 5].try_into().unwrap());
    assert!(made.values().any(|o| (o.x - gx).abs() < 1e-3 && (o.y - gy).abs() < 1e-3), "aimed at a piece");
    assert!(w.until_line().1.contains("is up"), "the line when she is done");

    // .den dojo: the training ring the same way, then three training dummies standing in it
    w.say(".den dojo");
    let mut frames = Vec::new();
    loop {
        let f = w.frame();
        if let Some(ServerOpcodeMessage::SMSG_MESSAGECHAT(m)) = &f.msg {
            if matches!(m.chat_type, SMSG_MESSAGECHAT_ChatType::MonsterSay { .. }) && m.message == den_lite::dojo::BUILT_DOJO {
                break;
            }
        }
        frames.push(f);
    }
    // she says it when she steps back to look; the dummies go up when she is done looking, with the line
    let (more, line) = w.until_line();
    assert!(line.contains("is up"), "{line}");
    frames.extend(more);
    let ring = srv.shared.den.lock().unwrap().props.len() - placed;
    assert!(ring >= 20, "the ring's pieces: {ring}");
    assert_eq!(objects(&frames).0.len(), ring);
    let dummies: Vec<u64> = frames
        .iter()
        .filter_map(|f| match &f.msg {
            Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) => Some(
                u.objects
                    .iter()
                    .filter_map(|o| match o {
                        Object::CreateObject { guid3, object_type: ObjectType::Unit, .. } => Some(guid3.guid()),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .filter(|g| (g >> 24) & 0xFF_FFFF == 31144)
        .collect();
    assert_eq!(dummies.len(), 3, "three training dummies");
    w.send(&CMSG_CREATURE_QUERY { creature: 31144, guid: Guid::new(dummies[0]) });
    let q = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CREATURE_QUERY_RESPONSE(r)) => Some(r.clone()),
        _ => None,
    });
    let found = q.found.expect("a training dummy");
    assert_eq!(found.name1, "Grandmaster's Training Dummy");
    assert!(!den_lite::companion::branded(&found.name1));

    // what needs a combat engine says so; the dummies come down; one undo takes the ring back
    w.say(".den dojo duel");
    assert!(w.until_line().1.contains("needs the full Den"));
    w.say(".dojo clear");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("3 dummies down"), "{line}");
    let mut gone = objects(&frames).1;
    gone.sort();
    let mut want = dummies.clone();
    want.sort();
    assert_eq!(gone, want);
    w.say(".build undo");
    let (frames, _) = w.until_line();
    assert_eq!(objects(&frames).1.len(), ring, "the ring undone in one");

    // the Den bar's Dojo panel asks for its line
    w.addon("OVMOD", "wuden\thello\t1/1\t1");
    let _ = until_den(&mut w, "status");
    w.addon("OVMOD", "wuden\tcmd\t1/1\tdojo status");
    let (_, line) = until_den(&mut w, "dojo");
    assert!(line.starts_with("server=lite;fly=0;speed=3;dummies=0;ring=1;building=0;combat=1;class=-"), "{line}");
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "dojo_ack" && l.detail.starts_with("fly ")), "the ack is in the events");
    assert!(evs.iter().any(|l| l.ev == "dojo" && l.detail.starts_with("built ")));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// One gameobject created alone in an update (the aids' own creates, build mode's look): its guid and the type the
/// client is told (GAMEOBJECT_BYTES_1's second byte: the block's last field).
fn single_go(f: &Frame) -> Option<(u64, u8)> {
    let b = &f.body;
    if f.opcode != 0x00A9 || b.len() < 12 || b[0..4] != [1, 0, 0, 0] || b[4] != 2 {
        return None;
    }
    let (mask, mut g, mut at) = (b[5], 0u64, 6);
    for i in 0..8 {
        if mask & (1 << i) != 0 {
            g |= (b[at] as u64) << (i * 8);
            at += 1;
        }
    }
    Some((g, b[b.len() - 3]))
}

/// Frames until a one-part addon answer `wubuild <name>`: the frames before it and its data.
/// The last whole `wubuild <name>` answer in frames already read (its parts joined; the state line is often more
/// than one part), or None.
fn aid_parts(frames: &[Frame], name: &str) -> Option<String> {
    let want = format!("OVMOD\twubuild\t{name}\t");
    let (mut parts, mut whole) = (Vec::new(), None);
    for f in frames {
        let Some(rest) = addon_text(f).and_then(|t| t.strip_prefix(&want).map(String::from)) else { continue };
        let Some((pn, data)) = rest.split_once('\t') else { continue };
        let Some((i, n)) = pn.split_once('/') else { continue };
        let (Ok(i), Ok(n)) = (i.parse::<usize>(), n.parse::<usize>()) else { continue };
        if i == 1 {
            parts.clear();
        }
        parts.push(data.to_string());
        if i == n && parts.len() == n {
            whole = Some(parts.concat());
        }
    }
    whole
}

fn until_aid(w: &mut World, name: &str) -> (Vec<Frame>, String) {
    let want = format!("OVMOD\twubuild\t{name}\t1/1\t");
    let mut got = Vec::new();
    loop {
        let f = w.frame();
        if let Some(d) = addon_text(&f).and_then(|t| t.strip_prefix(&want).map(String::from)) {
            return (got, d);
        }
        got.push(f);
    }
}

/// The editor's placement aids (gizmo.rs) over the wire: build mode shows props as goobers; a right-click on one
/// (CMSG_GAMEOBJ_USE) selects it and the gizmo's three flags come up; a right-click on a flag picks its axis; the
/// live preview's copy goes where the cursor's ray lands and Commit places the prop there; the exact click (the
/// ground-target cursor's CMSG_CAST_SPELL with a destination) places one where the click was, answered silently.
#[test]
fn the_editors_preview_gizmo_selection_and_exact_click() {
    use den_lite::gizmo::{self, Camera, PLACE_SPELL};
    let (srv, dir) = server("aids", HeroGender::Female);
    let mut w = into_world(&srv, 2);
    let s0 = Spot::default();
    w.send(&heartbeat(s0.x, s0.y, s0.z, 0.0));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tspawn 2413");
    let mut st = w.until_addon("wubuild", "state");
    while !st.contains("n=1;") {
        st = w.until_addon("wubuild", "state");
    }
    let id: u64 = st.split("sel=").nth(1).unwrap().split(',').next().unwrap().parse().unwrap();
    let chair = (0xF110u64 << 48) | (2413u64 << 24) | id;

    // build mode: the chair shown again as a goober (the client highlights it and lets you right-click it)
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tmode build");
    let (frames, mode) = until_aid(&mut w, "mode");
    assert_eq!(mode, "build");
    assert_eq!(frames.iter().filter_map(single_go).find(|g| g.0 == chair), Some((chair, 10)), "a goober in build mode");
    // under its build twin's entry, which the client asks about as a new entry: a goober with the chair's model and name
    // (the client keeps query answers and goes by them: the chair's own answer, type 7, would win)
    w.send(&CMSG_GAMEOBJECT_QUERY { entry_id: gizmo::twin(2413), guid: Guid::new(chair) });
    let f = loop {
        let f = w.frame();
        if f.opcode == GO_QUERY_RESPONSE {
            break f;
        }
    };
    assert_eq!(&f.body[0..12], &[gizmo::twin(2413).to_le_bytes(), 10u32.to_le_bytes(), 39u32.to_le_bytes()].concat()[..]);
    assert!(f.body[12..].starts_with(b"Wooden Chair\0"));
    assert!(frames.iter().any(|f| matches!(&f.msg, Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) if d.guid.guid() == chair)));
    // play mode: as it is again (a chair: type 7, right-click sits); then back to build mode
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tmode play");
    let (frames, mode) = until_aid(&mut w, "mode");
    assert_eq!(mode, "play");
    assert_eq!(frames.iter().filter_map(single_go).find(|g| g.0 == chair), Some((chair, 7)), "a chair in play mode");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tmode build");
    assert_eq!(until_aid(&mut w, "mode").1, "build");

    // the gizmo on; a right-click on the chair selects it: the addon is told to open its menu
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tgizmo on");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tselclear");
    w.until_addon("wubuild", "state");
    w.raw(gizmo::CMSG_GAMEOBJ_USE, &chair.to_le_bytes());
    let (frames, picked) = until_aid(&mut w, "picked");
    assert_eq!(picked, id.to_string());
    let (made, _) = objects(&frames);
    let flags: Vec<(u64, &Go)> = made.iter().filter(|(_, g)| (990_001..=990_003).contains(&g.entry)).map(|(k, g)| (*k, g)).collect();
    assert_eq!(flags.len(), 3, "the gizmo's three flags: {made:?}");
    let x = flags.iter().find(|f| f.1.entry == 990_001).unwrap();
    assert_eq!(x.1.display, 5913);
    assert!(x.1.x > s0.x + 3.0, "beside the chair, along x");
    assert!(w.until_addon("wubuild", "state").contains(&format!("sel={id},2413")));
    // a handle right-clicked: its axis grabbed (the next test drags it); its entry answers the query as a goober
    let y = flags.iter().find(|f| f.1.entry == 990_002).unwrap().0;
    w.raw(gizmo::CMSG_GAMEOBJ_USE, &y.to_le_bytes());
    assert_eq!(until_aid(&mut w, "axis").1, "y");
    w.send(&CMSG_GAMEOBJECT_QUERY { entry_id: 990_002, guid: Guid::new(y) });
    let q = w.until(|f| (f.opcode == GO_QUERY_RESPONSE).then(|| f.body.clone()));
    assert_eq!((u32::from_le_bytes(q[0..4].try_into().unwrap()), u32::from_le_bytes(q[4..8].try_into().unwrap()), u32::from_le_bytes(q[8..12].try_into().unwrap())), (990_002, 10, 7153));

    // the live preview: armed, the cursor's ray, Commit
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tpreview 2413");
    assert_eq!(until_aid(&mut w, "aim").1, format!("place,2413,{PLACE_SPELL}"));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\thover 0.1 -0.2 1.7778 12");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\twhere");
    let at: Vec<f32> = until_aid(&mut w, "ghost").1.split(',').map(|v| v.parse().unwrap()).collect();
    let here = den_lite::ovmod::Here { x: s0.x, y: s0.y, z: s0.z, o: 0.0 };
    let want = gizmo::project(&Camera { dist: 12.0, ..Camera::default() }, here, 0.1, -0.2, 1.7778, s0.z).unwrap();
    assert!((at[0] - want.0).abs() < 0.02 && (at[1] - want.1).abs() < 0.02, "{at:?} {want:?}");
    // Shift+click: placed, still armed for the exact click below (a plain click ends placing: the next test)
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcommit keep");
    let mut st = w.until_addon("wubuild", "state");
    while !st.contains("mine=2/") {
        st = w.until_addon("wubuild", "state");
    }
    let pos: Vec<f32> = st.split("pos=").nth(1).unwrap().split(';').next().unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    assert!((pos[0] - at[0]).abs() < 0.02 && (pos[1] - at[1]).abs() < 0.02, "placed where the copy was: {st}");

    // the exact click: Flare's cast with a destination, never cast; the prop placed there
    let mut b = vec![4u8];
    b.extend_from_slice(&PLACE_SPELL.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&gizmo::DEST_LOCATION.to_le_bytes());
    b.push(0);
    for v in [s0.x - 4.0, s0.y + 5.0, s0.z] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    w.raw(gizmo::CMSG_CAST_SPELL, &b);
    let fail = w.until(|f| (f.opcode == gizmo::SMSG_CAST_FAILED).then(|| f.body.clone()));
    assert_eq!(fail, vec![4, 0x07, 0x06, 0, 0, gizmo::SPELL_FAILED_DONT_REPORT], "silent: no message, no cooldown");
    let mut st = w.until_addon("wubuild", "state");
    while !st.contains("mine=3/") {
        st = w.until_addon("wubuild", "state");
    }
    assert!(st.contains(&format!("pos={:.2},{:.2},", s0.x - 4.0, s0.y + 5.0)), "{st}");

    // Esc: the copy goes, nothing armed
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tpreview off");
    assert_eq!(until_aid(&mut w, "aim").1, "off");
    let evs = std::fs::read_to_string(dir.join("den-lite.events")).unwrap_or_default();
    assert!(evs.contains("aid_click") && evs.contains("aid_use") && evs.contains("build_mode"), "{evs}");
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The editor's flow as the owner's video shows it (3 Nov), over the wire: a gallery pick arms the real prop, which
/// shows where the cursor is (no copy before that); one click places it and ends placing; the new prop is the
/// selection at once (its GAMEOBJECT_DYNAMIC glow in a values update, the handles and the halo, `placed`); a handle
/// right-clicked and the cursor moved drags the prop along that axis and a click drops it ("Moved ... yd along X",
/// one undo); a left click on empty ground (`selclear`) takes the glow off; snap out puts the view on Den Lite's
/// camera object (an invisible unit, PLAYER_FARSIGHT on it) and the same key gives it back.
#[test]
fn the_editors_pick_place_select_drag_and_snap_out() {
    use den_lite::gizmo::{self, GLOW};
    let (srv, dir) = server("flow", HeroGender::Female);
    let mut w = into_world(&srv, 2);
    let s0 = Spot::default();
    w.send(&heartbeat(s0.x, s0.y, s0.z, 0.0));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tmode build");
    assert_eq!(until_aid(&mut w, "mode").1, "build");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tgizmo on");

    // the pick: armed, nothing shown until the cursor is over the ground; the hover rides along with the pick
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tpreview 2413");
    let (frames, aim) = until_aid(&mut w, "aim");
    assert_eq!(aim, format!("place,2413,{}", gizmo::PLACE_SPELL));
    assert!(objects(&frames).0.is_empty(), "no copy in front of you");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\thover 0.15 -0.25 1.7778 12");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\twhere");
    let (frames, at) = until_aid(&mut w, "ghost");
    let at: Vec<f32> = at.split(',').map(|v| v.parse().unwrap()).collect();
    let copy: Vec<(u64, Go)> = objects(&frames).0.into_iter().filter(|(g, _)| gizmo::is_ghost(*g)).collect();
    assert_eq!(copy.len(), 1, "the real prop's copy under the cursor");
    assert_eq!((copy[0].1.entry, copy[0].1.display), (2413, 39));

    // one click: placed there, placing over, the new prop selected and glowing
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcommit");
    let (frames, placed) = until_aid(&mut w, "placed");
    let id: u64 = placed.parse().unwrap();
    let chair = (0xF110u64 << 48) | (2413u64 << 24) | id;
    let (made, gone) = objects(&frames);
    let real = made.get(&chair).expect("the chair made");
    assert!((real.x - at[0]).abs() < 0.02 && (real.y - at[1]).abs() < 0.02);
    assert!(gone.contains(&copy[0].0), "the copy goes once the real one is there");
    let mut glow = None;
    let mut handles = Vec::new();
    let mut frames2 = frames;
    // the state may come before "placed" (aid_send's order): look in what came already, else wait for it
    let st = match aid_parts(&frames2, "state") {
        Some(st) => st,
        None => w.until_addon("wubuild", "state"),
    };
    for f in &frames2 {
        if let Some((g, v)) = values(f) {
            if g == chair {
                glow = v.get(&(den_lite::raw::GAMEOBJECT_DYNAMIC as u16)).copied();
            }
        }
    }
    for (g, o) in objects(&frames2).0 {
        if (990_001..=990_003).contains(&(o.entry as u32)) {
            handles.push((g, o));
        }
    }
    assert_eq!(glow, Some(GLOW), "the selection glows (GAMEOBJECT_DYNAMIC)");
    assert_eq!(handles.len(), 3, "the three handles");
    assert!(st.contains(&format!("sel={id},2413")), "{st}");

    // grab the X handle and drag: the copy along x, then a click drops it there
    let xh = handles.iter().find(|h| h.1.entry == 990_001).unwrap().0;
    w.raw(gizmo::CMSG_GAMEOBJ_USE, &xh.to_le_bytes());
    assert_eq!(until_aid(&mut w, "aim").1, format!("axis,{id},x"));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\thover 0.3 -0.1 1.7778 12");
    let drag = until_aid(&mut w, "drag").1;
    assert!(drag.starts_with("x,"), "{drag}");
    let s: f32 = drag[2..].parse().unwrap();
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcommit");
    let moved = until_aid(&mut w, "moved").1;
    assert_eq!(moved, format!("x,{s:+.2}"));
    let mut st = w.until_addon("wubuild", "state");
    while !st.contains("pos=") {
        st = w.until_addon("wubuild", "state");
    }
    let pos: Vec<f32> = st.split("pos=").nth(1).unwrap().split(';').next().unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    assert!((pos[0] - (real.x + s)).abs() < 0.03 && (pos[1] - real.y).abs() < 0.03, "moved along x only: {st}");
    assert!(st.contains("undo=2;"), "one undo step for the drop: {st}");

    // a click on empty ground: deselected, the glow off
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tselclear");
    let off = w.until(|f| values(f).filter(|(g, _)| *g == chair).map(|(_, v)| v.get(&(den_lite::raw::GAMEOBJECT_DYNAMIC as u16)).copied()));
    assert_eq!(off, Some(0));

    // snap out: the camera object (an invisible unit) and the view on it; again: the view back, the object gone
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam zoom 12");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam snap");
    let (frames, cam) = until_aid(&mut w, "cam");
    assert!(cam.starts_with("far,35.0,1,"), "{cam}");
    let units = unit_creates(&frames);
    assert!(units.iter().any(|u| u.0 == den_lite::farsight::GUID && u.1 == den_lite::farsight::ENTRY as i32), "{units:?}");
    let far = frames.iter().filter_map(values).find(|(_, v)| v.contains_key(&(den_lite::raw::PLAYER_FARSIGHT as u16)));
    let (_, v) = far.expect("PLAYER_FARSIGHT set");
    let target = v[&(den_lite::raw::PLAYER_FARSIGHT as u16)] as u64 | (v[&(den_lite::raw::PLAYER_FARSIGHT as u16 + 1)] as u64) << 32;
    assert_eq!(target, den_lite::farsight::GUID);
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam pan fwd 6");
    let (frames, _) = until_aid(&mut w, "cam");
    assert!(frames.iter().any(|f| f.opcode == den_lite::flight::SMSG_MONSTER_MOVE), "the view glides");
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam snap");
    let (frames, cam) = until_aid(&mut w, "cam");
    assert_eq!(cam, "near");
    let back = frames.iter().filter_map(values).find(|(_, v)| v.contains_key(&(den_lite::raw::PLAYER_FARSIGHT as u16))).unwrap().1;
    assert_eq!(back[&(den_lite::raw::PLAYER_FARSIGHT as u16)], 0);
    assert!(frames.iter().any(|f| matches!(&f.msg, Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) if d.guid.guid() == den_lite::farsight::GUID)));
    // the wheel past the client's limit: the same object, further each notch
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam far 2");
    assert!(until_aid(&mut w, "cam").1.starts_with("far,"));
    w.addon("OVMOD", "wubuild\tcmd\t1/1\tcam far -2");
    assert_eq!(until_aid(&mut w, "cam").1, "near");
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A unit's create in some frames: (guid, entry, health, max health, faction).
fn unit_creates(frames: &[Frame]) -> Vec<(u64, i32, i32, i32, i32)> {
    let mut v = Vec::new();
    for f in frames {
        if let Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) = &f.msg {
            for o in &u.objects {
                if let Object::CreateObject { guid3, mask2: UpdateMask::Unit(m), object_type: ObjectType::Unit, .. } = o {
                    v.push((guid3.guid(), m.object_entry().unwrap_or(0), m.unit_health().unwrap_or(-1), m.unit_maxhealth().unwrap_or(-1), m.unit_factiontemplate().unwrap_or(-1)));
                }
            }
        }
    }
    v
}

/// A values update's (guid, field -> value) read by hand (SMSG_UPDATE_OBJECT, one block of type 0).
fn values(f: &Frame) -> Option<(u64, HashMap<u16, u32>)> {
    if f.opcode != 0x00A9 || f.body.get(4) != Some(&0) {
        return None;
    }
    let b = &f.body;
    let mut at = 5;
    let guid = den_lite::combat::read_packed(b, &mut at)?;
    let blocks = *b.get(at)? as usize;
    at += 1;
    let masks: Vec<u32> = (0..blocks).map(|k| u32::from_le_bytes(b[at + 4 * k..at + 4 * k + 4].try_into().unwrap())).collect();
    at += 4 * blocks;
    let mut out = HashMap::new();
    for (k, m) in masks.iter().enumerate() {
        for bit in 0..32 {
            if m & (1 << bit) != 0 {
                out.insert((k * 32 + bit) as u16, u32::from_le_bytes(b[at..at + 4].try_into().unwrap()));
                at += 4;
            }
        }
    }
    Some((guid, out))
}

/// CMSG_CAST_SPELL at a unit, as the game writes it.
fn cast_at(count: u8, spell: u32, unit: u64) -> Vec<u8> {
    let mut b = vec![count];
    b.extend_from_slice(&spell.to_le_bytes());
    b.push(0);
    b.extend_from_slice(&2u32.to_le_bytes());
    den_lite::raw::packed_guid(&mut b, unit);
    b
}

#[test]
fn the_class_picker_and_the_dummies_take_spells_and_swings() {
    let (srv, dir) = server("combat", HeroGender::Female);
    // the player's own tables, as the launcher copies them (made-up rows here: Fireball, Fire Blast, a buff...)
    den_lite::spells::fake::write_all(&dir);
    let mut w = into_world(&srv, 0);
    let (mx, my, mz) = (16226.2f32, 16257.0f32, 13.25f32);
    w.send(&heartbeat(mx, my, mz, 0.0));

    // .den class mage: the class's fields, every spell it learns, its bar, at once; kept in the Dens folder
    w.say(".den class mage 80");
    let mut fields = None;
    let spells = w.until(|f| {
        if let Some((g, v)) = values(f) {
            if g == den_lite::world::PLAYER_GUID {
                fields = Some(v);
            }
        }
        (f.opcode == 0x012A).then(|| f.body.clone())
    });
    let fields = fields.expect("the class's fields first");
    assert_eq!(fields[&den_lite::combat::field::BYTES_0] >> 8 & 0xFF, 8, "a mage");
    assert_eq!(fields[&den_lite::combat::field::LEVEL], 80);
    assert_eq!(fields[&den_lite::combat::field::MAXPOWER1], 11620, "mana");
    let n = u16::from_le_bytes([spells[1], spells[2]]) as usize;
    let ids: Vec<u32> = (0..n).map(|k| u32::from_le_bytes(spells[3 + 6 * k..7 + 6 * k].try_into().unwrap())).collect();
    assert!(ids.contains(&143) && !ids.contains(&133) && ids.contains(&2136) && ids.contains(&6603), "{ids:?}");
    let bar = w.until(|f| (f.opcode == 0x0129).then(|| f.body.clone()));
    assert_eq!(bar[0], 1, "a change of bars");
    assert_eq!(u32::from_le_bytes(bar[1..5].try_into().unwrap()), 143, "Fireball first");
    let line = w.until_line().1;
    assert!(line.starts_with("You are a level 80 Fire Mage now"), "{line}");
    let kept = std::fs::read_to_string(dir.join(den_lite::classes::FILE)).unwrap();
    assert!(kept.contains("\"mage\""));

    // a training dummy: attackable (neutral faction 7), huge health
    w.say(".den dojo dummies 1");
    let (frames, _) = w.until_line();
    let made = unit_creates(&frames);
    assert_eq!(made.len(), 1);
    let (dummy, _, hp, max, faction) = made[0];
    assert_eq!((hp, max, faction), (5_000_000, 5_000_000, 7));

    // an unknown spell is refused; Fireball: start (2.5 s), go at the dummy, its damage, its health down
    w.raw(0x12E, &cast_at(1, 78, dummy));
    let f = w.until(|f| (f.opcode == 0x0130).then(|| f.body.clone()));
    assert_eq!((f[0], f[5]), (1, 63), "not known");
    let t0 = std::time::Instant::now();
    w.raw(0x12E, &cast_at(2, 143, dummy));
    let start = w.until(|f| (f.opcode == 0x0131).then(|| f.body.clone()));
    assert_eq!(start[4], 2, "the cast count");
    assert_eq!(u32::from_le_bytes(start[13..17].try_into().unwrap()), 2500, "the cast time from the table");
    let go = w.until(|f| (f.opcode == 0x0132).then(|| f.body.clone()));
    assert!(t0.elapsed() >= Duration::from_millis(2300), "after the cast time: {:?}", t0.elapsed());
    assert_eq!(go[17], 1, "one unit hit");
    assert_eq!(u64::from_le_bytes(go[18..26].try_into().unwrap()), dummy);
    let log = w.until(|f| (f.opcode == 0x0250).then(|| f.body.clone()));
    let mut at = 0;
    assert_eq!(den_lite::combat::read_packed(&log, &mut at), Some(dummy));
    den_lite::combat::read_packed(&log, &mut at);
    let dmg = u32::from_le_bytes(log[at + 4..at + 8].try_into().unwrap());
    assert!(dmg > 800, "Fireball's damage at 80: {dmg}");
    let left = w.until(|f| values(f).filter(|(g, _)| *g == dummy).map(|(_, v)| v[&den_lite::combat::field::HEALTH]));
    assert_eq!(left, 5_000_000 - dmg, "its health drops");

    // moving stops a cast
    w.raw(0x12E, &cast_at(3, 143, dummy));
    w.until(|f| (f.opcode == 0x0131).then_some(()));
    let mut mv = vec![0x01, 0x01];
    mv.extend_from_slice(&1u32.to_le_bytes());
    mv.extend_from_slice(&0u16.to_le_bytes());
    mv.extend_from_slice(&2000u32.to_le_bytes());
    for v in [mx, my, mz, 0.0f32] {
        mv.extend_from_slice(&v.to_le_bytes());
    }
    mv.extend_from_slice(&0u32.to_le_bytes());
    w.raw(0x0B5, &mv);
    let fail = w.until(|f| (f.opcode == 0x0133).then(|| f.body.clone()));
    assert_eq!(*fail.last().unwrap(), 40, "interrupted");
    w.send(&heartbeat(mx, my, mz, 0.0));

    // the auto attack: start, swings, stop (the dummy is 5 yards ahead: step up to it first)
    w.send(&heartbeat(mx + 3.5, my, mz, 0.0));
    w.raw(0x141, &dummy.to_le_bytes());
    let st = w.until(|f| (f.opcode == 0x0143).then(|| f.body.clone()));
    assert_eq!(u64::from_le_bytes(st[8..16].try_into().unwrap()), dummy);
    let swing = w.until(|f| (f.opcode == 0x014A).then(|| f.body.clone()));
    let mut at = 4;
    assert_eq!(den_lite::combat::read_packed(&swing, &mut at), Some(den_lite::world::PLAYER_GUID));
    assert_eq!(den_lite::combat::read_packed(&swing, &mut at), Some(dummy));
    assert!(u32::from_le_bytes(swing[at..at + 4].try_into().unwrap()) > 0, "a swing's damage");
    w.raw(0x142, &[]);
    w.until(|f| (f.opcode == 0x0144).then_some(()));

    // reset: the dummy full again
    w.say(".den dojo reset");
    let full = w.until(|f| values(f).filter(|(g, _)| *g == dummy).map(|(_, v)| v[&den_lite::combat::field::HEALTH]));
    assert_eq!(full, 5_000_000);
    assert!(w.until_line().1.starts_with("Reset:"));

    // the other dummies: a pack of five, the boss (it falls), the wounded one (friendly)
    w.say(".den dojo pack");
    let (frames, line) = w.until_line();
    assert_eq!(unit_creates(&frames).len(), 5, "{line}");
    w.say(".den dojo healer");
    let (frames, _) = w.until_line();
    let healer = unit_creates(&frames);
    assert_eq!(healer.len(), 1);
    assert_eq!(healer[0].4, 35, "friendly");
    assert!(healer[0].2 < healer[0].3, "it starts hurt");
    w.say(".den dojo parse 3");
    assert!(w.until_line().1.contains("3 minute parse"));
    w.say(".den dojo best");
    assert!(w.until_line().1.starts_with("No bests yet"));
    w.say(".den class bard");
    assert!(w.until_line().1.starts_with("No class called 'bard'"));

    // every mount: in the spellbook at once; cast one: its model and speed; off again
    w.say(".den mounts");
    let spells = w.until(|f| (f.opcode == 0x012A).then(|| f.body.clone()));
    let n = u16::from_le_bytes([spells[1], spells[2]]) as usize;
    let ids: Vec<u32> = (0..n).map(|k| u32::from_le_bytes(spells[3 + 6 * k..7 + 6 * k].try_into().unwrap())).collect();
    assert!(ids.contains(&458) && ids.contains(&143), "the mounts and the class's spells: {ids:?}");
    assert!(w.until_line().1.starts_with("1 mounts in your spellbook"));
    w.raw(0x12E, &[9, 0xCA, 0x01, 0, 0, 0, 0, 0, 0, 0]);
    let on = w.until(|f| values(f).filter(|(g, v)| *g == den_lite::world::PLAYER_GUID && v.contains_key(&69)).map(|(_, v)| v));
    assert_eq!(on[&69], 987, "the horse's model");
    let run = w.until(|f| (f.opcode == 0x0E2).then(|| f.body.clone()));
    assert_eq!(f32::from_le_bytes(run[7..11].try_into().unwrap()), 14.0);
    w.say(".den mounts list");
    assert_eq!(w.until_line().1, "Mounts, page 1 of 1: Brown Horse.");
    w.raw(0x375, &[]);
    let off = w.until(|f| values(f).filter(|(g, v)| *g == den_lite::world::PLAYER_GUID && v.contains_key(&69)).map(|(_, v)| v));
    assert_eq!(off[&69], 0, "off the horse");
    assert!(std::fs::metadata(dir.join("den-lite.mounts")).is_ok());
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "class" && l.detail.starts_with("mage 80")));
    assert!(evs.iter().any(|l| l.ev == "cast" && l.detail.starts_with("143 ")));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A companion move written by hand (flight.rs sky_body): (flags, duration, points).
fn sky_move(f: &Frame) -> Option<(u32, u32, Vec<[f32; 3]>)> {
    if f.opcode != 0x00DD {
        return None;
    }
    let b = &f.body;
    let mut at = 0;
    if den_lite::combat::read_packed(b, &mut at)? != den_lite::companion::GUID {
        return None;
    }
    at += 1 + 12 + 4;
    if *b.get(at)? != 0 {
        return None;
    }
    at += 1;
    let flags = u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?);
    if flags & 0x42000 == 0 {
        return None;
    }
    let ms = u32::from_le_bytes(b.get(at + 4..at + 8)?.try_into().ok()?);
    let n = u32::from_le_bytes(b.get(at + 8..at + 12)?.try_into().ok()?) as usize;
    let f = |k: usize| f32::from_le_bytes(b[k..k + 4].try_into().unwrap());
    let pts = (0..n).map(|i| at + 12 + 12 * i).map(|k| [f(k), f(k + 4), f(k + 8)]).collect();
    Some((flags, ms, pts))
}

#[test]
fn the_companion_flies_with_you_lands_with_you_and_walks_around_props() {
    let (srv, dir) = server_with("sky", HeroGender::Female, true);
    let (key, _, address) = login(srv.login, "DEN", "den").expect("login");
    let mut w = World::connect(&address, "DEN", key, 0);
    w.send(&CMSG_CHAR_ENUM {});
    let ch = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CHAR_ENUM(e)) => Some(e.characters[0].guid),
        _ => None,
    });
    w.send(&CMSG_PLAYER_LOGIN { guid: ch });
    assert_eq!(w.until_she_says().2, den_lite::companion::GREET);
    let (mx, my, mz) = (16226.2f32, 16257.0f32, 13.25f32);
    w.send(&heartbeat(mx, my, mz, 0.0));
    std::thread::sleep(Duration::from_millis(2500));

    // a prop right in her way: she walks around it, never through it
    let a = srv.shared.companion.lock().unwrap().pos(u64::MAX);
    let obj = den::find_objects("wooden chair").into_iter().next().expect("a chair");
    srv.shared.den.lock().unwrap().stamp(&den::one_piece(obj), a.x, a.y, mz, 0.0);
    let prop = srv.shared.den.lock().unwrap().props.last().unwrap().obj.clone();
    let b = (2.0 * prop.x - a.x, 2.0 * prop.y - a.y);
    let off = (0.75 * std::f32::consts::PI).sin_cos();
    let p2 = (b.0 - off.1 * 2.5, b.1 - off.0 * 2.5);
    w.send(&heartbeat(p2.0, p2.1, mz, 0.0));
    let (flags, _, pts) = w.until(|f| sky_move(f));
    assert_eq!(flags & 0x2000, 0, "on foot");
    assert!(pts.len() >= 2, "a waypoint around the chair: {pts:?}");
    for q in &pts {
        assert!(((q[0] - prop.x).powi(2) + (q[1] - prop.y).powi(2)).sqrt() > 0.6, "never in the chair: {q:?}");
    }
    let end = pts.last().unwrap();
    assert!(((end[0] - p2.0).powi(2) + (end[1] - p2.1).powi(2)).sqrt() >= 1.99, "never on top of you");

    // fly: up in the air she flies too (told she flies, then a curved flight to above your height)
    w.say(".den fly on");
    w.until(|f| (f.opcode == 0x343).then_some(()));
    let up = mz + 12.0;
    w.send(&heartbeat(p2.0, p2.1, up, 0.0));
    w.until(|f| (f.opcode == 0x422).then_some(()));
    w.until(|f| (f.opcode == 0x4D3).then_some(()));
    let (flags, ms, pts) = w.until(|f| sky_move(f));
    assert_eq!(flags, 0x42000, "Flying | Catmullrom");
    assert_eq!(pts.len(), 2);
    let last = pts[1];
    assert!((last[2] - (up + 0.8)).abs() < 0.01, "above your height: {last:?}");
    assert!(ms >= 300);
    // you fly on: she follows in the air
    w.send(&heartbeat(p2.0 + 15.0, p2.1, up + 5.0, 0.0));
    let (_, _, pts) = w.until(|f| sky_move(f).filter(|m| m.0 & 0x2000 != 0));
    assert!((pts[1][0] - (p2.0 + 15.0 - 1.77)).abs() < 0.1 && pts[1][2] > up + 5.0);
    // you land: she lands beside you (a flight down to the ground), then walks again
    w.send(&heartbeat(p2.0 + 15.0, p2.1, mz, 0.0));
    let (flags, _, pts) = w.until(|f| sky_move(f));
    assert_eq!(flags, 0x2000);
    assert!((pts[0][2] - 13.2).abs() < 0.2, "on the ground: {pts:?}");
    w.until(|f| (f.opcode == 0x423).then_some(()));
    w.until(|f| (f.opcode == 0x4D4).then_some(()));
    // the events file is written a moment later: wait up to 3 s for both lines
    let seen = |evs: &[den_lite::events::Line]| evs.iter().any(|l| l.ev == "companion" && l.detail == "flies") && evs.iter().any(|l| l.ev == "companion" && l.detail == "lands");
    let mut evs = den_lite::events::read(&dir);
    for _ in 0..30 {
        if seen(&evs) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        evs = den_lite::events::read(&dir);
    }
    assert!(seen(&evs), "{:?}", evs.iter().map(|l| (&l.ev, &l.detail)).collect::<Vec<_>>());
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn any_items_look_goes_on_at_once_and_is_kept() {
    let (srv, dir) = server("gear", HeroGender::Male);
    // the player's own item table, as the launcher copies it (made-up rows: id, class, subclass, sound, material,
    // display, inventory type, sheath)
    let rows = vec![vec![19019, 2, 7, 0, 1, 30606, 13, 3], vec![32837, 2, 7, 0, 1, 45479, 21, 3], vec![32838, 2, 7, 0, 1, 45481, 22, 3]];
    std::fs::write(dir.join(den_lite::gear::ITEM_TABLE), den_lite::spells::fake::table(8, &rows, &[])).unwrap();
    let mut w = into_world(&srv, 0);
    let me = den_lite::world::PLAYER_GUID;
    let mainhand = den_lite::gear::VISIBLE_ITEM + 30;
    w.say(".den gear 19019");
    let v = w.until(|f| values(f).filter(|(g, _)| *g == me).map(|(_, v)| v));
    assert_eq!((v[&mainhand], v[&(den_lite::gear::VISIBLE_ITEM + 6)]), (19019, 4334), "the weapon on, the shirt still on");
    assert!(w.until_line().1.starts_with("Your look: mainhand 19019"));
    w.say(".den gear warglaives");
    let v = w.until(|f| values(f).filter(|(g, _)| *g == me).map(|(_, v)| v));
    assert_eq!((v[&mainhand], v[&(mainhand + 2)]), (32837, 32838), "both hands");
    w.until_line();
    w.say(".den gear list mainhand");
    let line = w.until_line().1;
    assert!(line.starts_with("mainhand items, page 1 of 1: 19019 32837"), "{line}");
    w.say(".den gear head 999");
    let v = w.until(|f| values(f).filter(|(g, _)| *g == me).map(|(_, v)| v));
    assert_eq!(v[&den_lite::gear::VISIBLE_ITEM], 999, "a slot and a number need no table");
    w.until_line();
    let kept = std::fs::read_to_string(dir.join(den_lite::gear::FILE)).unwrap();
    assert!(kept.contains("32838") && kept.contains("\"head\""), "{kept}");
    w.say(".den gear reset");
    let v = w.until(|f| values(f).filter(|(g, _)| *g == me).map(|(_, v)| v));
    assert_eq!((v[&mainhand], v[&den_lite::gear::VISIBLE_ITEM]), (0, 0));
    assert!(w.until_line().1.starts_with("Back in your own outfit"));
    w.say(".den gear sparkly");
    assert!(w.until_line().1.starts_with("Usage: .den gear"));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// System lines until one starts with `prefix` (other lines, such as the companion staying behind, are skipped);
/// every frame on the way and the line.
fn until_said(w: &mut World, prefix: &str) -> (Vec<Frame>, String) {
    let mut all = Vec::new();
    loop {
        let (frames, line) = w.until_line();
        all.extend(frames);
        if line.starts_with(prefix) {
            return (all, line);
        }
    }
}

fn f32_at(b: &[u8], i: usize) -> f32 {
    f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Den anywhere (places.rs, owner 3 Nov): `.den go` on the same map is a near teleport the game answers, to another
/// map a far one (TRANSFER_PENDING, NEW_WORLD, the game's WORLDPORT_ACK, then everything again: the player's own
/// create with the self flag, the time sync, the world states, and no Den objects on a map the Den isn't on);
/// `.den here` moves the Den, keeping the old place's props in its own file, and props go down at the new place.
#[test]
fn the_den_goes_anywhere_near_and_far_and_each_place_keeps_its_den() {
    let (srv, dir) = server("places", HeroGender::Female);
    let mut w = into_world(&srv, 0);
    let me = den_lite::world::PLAYER_GUID;
    w.say(".den place chair");
    let (frames, _) = w.until_line();
    let (made, _) = objects(&frames);
    let chair = *made.keys().next().expect("the chair on the island");
    // near: Orgrimmar is on the island's map
    w.say(".den go orgrimmar");
    let (frames, _) = until_said(&mut w, "Off to Orgrimmar");
    let tp = frames.iter().find(|f| f.opcode == 0x0C7).expect("MSG_MOVE_TELEPORT_ACK");
    assert_eq!(&tp.body[..2], &[0x01, 0x01], "the player's packed guid");
    let counter = u32::from_le_bytes([tp.body[2], tp.body[3], tp.body[4], tp.body[5]]);
    assert_eq!((f32_at(&tp.body, 16), f32_at(&tp.body, 20)), (1629.36, -4373.39));
    assert!(!frames.iter().any(|f| f.opcode == 0x03E), "no loading screen on the same map");
    let mut ack = vec![0x01, 0x01];
    ack.extend_from_slice(&counter.to_le_bytes());
    ack.extend_from_slice(&1000u32.to_le_bytes());
    w.raw(0x0C7, &ack);
    until_said(&mut w, "You are at Orgrimmar");
    // far: Stormwind is on another map
    w.say(".den go stormwind");
    let (frames, _) = until_said(&mut w, "Off to Stormwind");
    assert!(frames.iter().any(|f| f.opcode == 0x03F && f.body == 0u32.to_le_bytes()), "SMSG_TRANSFER_PENDING, map 0");
    let nw = frames.iter().find(|f| f.opcode == 0x03E).expect("SMSG_NEW_WORLD");
    assert_eq!((&nw.body[..4], f32_at(&nw.body, 4)), (&0u32.to_le_bytes()[..], -8833.38));
    assert!(f32_at(&nw.body, 12) > 94.0, "a little above the ground: the game lands on its own");
    w.raw(0x0DC, &[]);
    let (frames, _) = until_said(&mut w, "You are at Stormwind");
    assert_eq!(player_created(&frames, me), Some(true), "the player's own create again, with the self flag");
    assert!(frames.iter().any(|f| f.opcode == 0x0390), "the time sync again");
    assert!(objects(&frames).0.is_empty(), "the Den's props stay on the island's map");
    // make my Den here: the island's chair goes, kept in its place's file
    w.send(&heartbeat(-8830.0, 630.0, 94.0, 0.0));
    w.say(".den here");
    let (frames, line) = until_said(&mut w, "Your Den is here now: Stormwind");
    assert!(objects(&frames).1.contains(&chair), "{line}");
    assert!(dir.join("places").join("m1_16226_16257.den.json").is_file());
    w.say(".den place chair");
    let (frames, _) = w.until_line();
    let (made, _) = objects(&frames);
    let c = made.values().next().expect("a chair in Stormwind");
    assert!((c.x + 8827.0).abs() < 1.0 && (c.y - 630.0).abs() < 1.0, "3 yards ahead, at the new place");
    w.say(".den places");
    let (_, line) = until_said(&mut w, "Your Den is at Stormwind");
    assert!(line.contains("Dens you kept at other places"), "{line}");
    // back to the island: no Den objects there now (the Den is in Stormwind)
    w.say(".den go island");
    until_said(&mut w, "Off to your first Den");
    w.raw(0x0DC, &[]);
    let (frames, _) = until_said(&mut w, "You are at your first Den");
    assert!(objects(&frames).0.is_empty());
    w.say(".den size city");
    let (_, line) = until_said(&mut w, "Your Den is size 7 of 7");
    assert!(line.contains("city-size"), "{line}");
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tables of the sandbox tests below, as the launcher copies the player's own (made-up rows): the sample's
/// spells, a kick (Counterspell), Summon Imp, and two talent trees for the mage (fire: a five-rank passive, then
/// Blast Wave; arcane: a passive), and their TalentTab rows.
fn combat_tables(dir: &std::path::Path) {
    use den_lite::spells::fake;
    let _ = std::fs::create_dir_all(dir);
    let mut rows = fake::sample();
    rows.push(fake::Row { id: 2139, name: "Counterspell".into(), level: 24, cast_index: 1, recovery: 24_000, school: 64, effects: vec![(68, 0, 1, 6, 0, 0, 0, 0)], ..Default::default() });
    rows.push(fake::Row { id: 688, name: "Summon Imp".into(), level: 1, cast_index: 1, school: 32, effects: vec![(56, 0, 1, 1, 0, 0, 0, 416)], ..Default::default() });
    rows.push(fake::Row { id: 9100, name: "Blast Wave".into(), level: 30, cast_index: 1, gcd: 1500, school: 4, effects: vec![(2, 600, 100, 22, 0, 0, 0, 0)], ..Default::default() });
    rows.push(fake::Row { id: 9300, name: "Arcane Focus".into(), level: 10, attributes: 0x40, cast_index: 1, school: 64, effects: vec![(6, 1, 1, 1, 57, 0, 0, 0)], ..Default::default() });
    for k in 0..5u32 {
        rows.push(fake::Row { id: 7000 + k, name: "Burning Soul".into(), level: 10, attributes: 0x40, cast_index: 1, school: 4, effects: vec![(6, 1, 1, 1, 57, 0, 0, 0)], ..Default::default() });
    }
    let mut abilities = fake::ABILITIES.to_vec();
    abilities.extend([(237, 2139, 128, 0), (8, 688, 128, 0), (8, 9100, 128, 0), (237, 9300, 128, 0)]);
    std::fs::write(dir.join(den_lite::spells::SPELL), fake::spells(&rows)).unwrap();
    std::fs::write(dir.join(den_lite::spells::SKILL_LINE_ABILITY), fake::abilities(&abilities)).unwrap();
    std::fs::write(dir.join(den_lite::spells::CAST_TIMES), fake::four(&fake::CAST_TIMES)).unwrap();
    std::fs::write(dir.join(den_lite::spells::DURATIONS), fake::four(&fake::DURATIONS)).unwrap();
    std::fs::write(dir.join(den_lite::spells::RADIUS), fake::four(&[])).unwrap();
    let filler: &[u32] = &[7000, 7001, 7002, 7003, 7004];
    let talents = den_lite::specs::fake::talents(&[(1, 41, 0, 0, filler, 0, 0), (2, 41, 1, 1, &[9100], 0, 0), (10, 61, 0, 0, &[9300], 0, 0)]);
    std::fs::write(dir.join(den_lite::specs::TALENT), talents).unwrap();
    std::fs::write(dir.join(den_lite::specs::TALENT_TAB), den_lite::specs::fake::tabs(&[(61, 128, 0), (41, 128, 1), (81, 128, 2)])).unwrap();
    den_lite::spells::forget();
}

fn spell_ids(b: &[u8]) -> Vec<u32> {
    let n = u16::from_le_bytes([b[1], b[2]]) as usize;
    (0..n).map(|k| u32::from_le_bytes(b[3 + 6 * k..7 + 6 * k].try_into().unwrap())).collect()
}

fn button(bar: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(bar[1 + 4 * i..5 + 4 * i].try_into().unwrap())
}

/// Specs and bars (owner 3 Nov: "spellbook placement loadout instant on bars"): `.den class mage fire 80` sends the
/// class's fields (the four extra bars shown), the spec's spellbook (its talents' spells), every bar (the rotation
/// first, the buff on the second right bar) and the talents (SMSG_TALENTS_INFO); `.den spec arcane` another spec
/// at once (fire's talent ability gone); a bar saved by name comes back after a change; all kept in the Dens folder.
#[test]
fn a_class_and_spec_fill_the_spellbook_talents_and_every_bar_and_loadouts_come_back() {
    let (srv, dir) = server("spec", HeroGender::Male);
    combat_tables(&dir);
    let mut w = into_world(&srv, 0);
    w.send(&heartbeat(16226.2, 16257.0, 13.25, 0.0));
    w.say(".den class mage fire 80");
    let me = den_lite::world::PLAYER_GUID;
    let mut fields = None;
    let spells = w.until(|f| {
        if let Some((g, v)) = values(f) {
            if g == me && v.contains_key(&den_lite::combat::field::BYTES_0) {
                fields = Some(v);
            }
        }
        (f.opcode == 0x012A).then(|| f.body.clone())
    });
    let fields = fields.expect("the class's fields");
    assert_eq!(fields[&153], 0x000F_0000, "the four extra action bars shown");
    let ids = spell_ids(&spells);
    assert!(ids.contains(&9100) && ids.contains(&7004) && ids.contains(&143), "fire's talents and the class's spells: {ids:?}");
    let bar = w.until(|f| (f.opcode == 0x0129).then(|| f.body.clone()));
    assert_eq!(bar[0], 1, "a change of bars");
    assert_eq!(button(&bar, 0), 143, "Fireball first");
    assert_eq!(button(&bar, 36), 1459, "the buff on the second right bar");
    assert!((0..144).filter(|i| button(&bar, *i) != 0).count() >= 6);
    let t = w.until(|f| (f.opcode == 0x04C0).then(|| f.body.clone()));
    assert_eq!((t[0], t[5], t[6]), (0, 1, 0), "a player's talents, one spec, the first");
    let n = t[7] as usize;
    let taken: Vec<(u32, u8)> = (0..n).map(|k| (u32::from_le_bytes(t[8 + 5 * k..12 + 5 * k].try_into().unwrap()), t[12 + 5 * k])).collect();
    assert!(taken.contains(&(1, 4)) && taken.contains(&(2, 0)) && taken.contains(&(10, 0)), "five ranks, Blast Wave, arcane's: {taken:?}");
    let line = until_said(&mut w, "You are a level 80").1;
    assert!(line.starts_with("You are a level 80 Fire Mage now") && line.contains("7 talent points"), "{line}");

    // another spec: its own spellbook and talents
    w.say(".den spec arcane");
    let spells = w.until(|f| (f.opcode == 0x012A).then(|| f.body.clone()));
    let ids = spell_ids(&spells);
    assert!(!ids.contains(&9100) && ids.contains(&9300), "fire's talent ability isn't arcane's: {ids:?}");
    assert!(until_said(&mut w, "Arcane Mage now").1.contains("every bar filled"));

    // a bar loadout: saved, a button changed, put back
    w.say(".den bars save aoe");
    let saved = w.until(|f| (f.opcode == 0x0129).then(|| f.body.clone()));
    assert_eq!(until_said(&mut w, "Saved").1, "Saved your bar as 'aoe'.");
    w.raw(0x128, &[0, 0x58, 0x08, 0, 0]);
    w.say(".den bars use aoe");
    let back = w.until(|f| (f.opcode == 0x0129).then(|| f.body.clone()));
    assert_eq!(&back[1..], &saved[1..], "the saved bar, every button");
    assert_eq!(until_said(&mut w, "Your 'aoe'").1, "Your 'aoe' bar is on.");
    w.say(".den bars");
    assert!(w.until_line().1.contains("aoe"));
    let kept = std::fs::read_to_string(dir.join(den_lite::classes::FILE)).unwrap();
    assert!(kept.contains("\"arcane\"") && kept.contains("\"aoe\""), "{kept}");
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "class" && l.detail.starts_with("mage 80 ") && l.detail.contains(" fire ")));
    assert!(evs.iter().any(|l| l.ev == "spec" && l.detail.starts_with("mage arcane")));
    assert!(evs.iter().any(|l| l.ev == "bars" && l.detail == "save aoe"));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Gear sets (owner 3 Nov: "gear sets selector"): `.den gearset t10` puts the class's tier set on (its pieces, the
/// slots it has none for from the same era, a staff and a wand), in every visible slot at once; the weapons are
/// real items (their creates, the inventory fields) the game can ask about (SMSG_ITEM_QUERY_SINGLE_RESPONSE, read
/// by the message crate); saved by name, off, and back on.
#[test]
fn a_gear_set_changes_every_visible_slot_and_its_weapons_are_real_items() {
    let (srv, dir) = server("gearset", HeroGender::Female);
    combat_tables(&dir);
    std::fs::write(dir.join(den_lite::gearsets::ITEM_SET), den_lite::gearsets::fake::sets(&[(803, "Bloodmage's Regalia", &[51281, 51284, 51283, 51280, 51282])])).unwrap();
    std::fs::write(
        dir.join(den_lite::gear::ITEM_TABLE),
        den_lite::gearsets::fake::items(&[
            (51281, 4, 1, 200, 1), (51284, 4, 1, 201, 3), (51283, 4, 1, 202, 20), (51280, 4, 1, 203, 10), (51282, 4, 1, 204, 7),
            (50000, 4, 1, 300, 6), (50731, 2, 10, 500, 17), (50033, 2, 19, 502, 26),
        ]),
    )
    .unwrap();
    let mut w = into_world(&srv, 0);
    let me = den_lite::world::PLAYER_GUID;
    let vis = den_lite::gear::VISIBLE_ITEM;
    w.say(".den class mage fire 80");
    until_said(&mut w, "You are a level 80");
    w.say(".den gearset");
    let line = w.until_line().1;
    assert!(line.contains("t10") && line.contains("Fire"), "{line}");
    w.say(".den gearset t10");
    let v = w.until(|f| values(f).filter(|(g, v)| *g == me && v.contains_key(&vis)).map(|(_, v)| v));
    assert_eq!((v[&vis], v[&(vis + 4)], v[&(vis + 8)]), (51281, 51284, 51283), "the set's head, shoulders, chest");
    assert_eq!(v[&(vis + 10)], 50000, "the waist it has no piece for: the era's cloth");
    assert_eq!((v[&(vis + 30)], v[&(vis + 34)]), (50731, 50033), "a staff and a wand");
    let mut made = Vec::new();
    let inv = w.until(|f| {
        if let Some(ServerOpcodeMessage::SMSG_UPDATE_OBJECT(u)) = &f.msg {
            for o in &u.objects {
                if let Object::CreateObject { guid3, mask2: UpdateMask::Item(m), .. } = o {
                    made.push((guid3.guid(), m.object_entry().unwrap_or(0)));
                }
            }
        }
        values(f).filter(|(g, v)| *g == me && v.contains_key(&(324 + 30))).map(|(_, v)| v)
    });
    let staff = made.iter().find(|m| m.1 == 50731).expect("the staff made real").0;
    assert!(made.iter().any(|m| m.1 == 50033), "{made:?}");
    assert_eq!(inv[&(324 + 30)], staff as u32, "the main hand holds it");
    let line = until_said(&mut w, "Tier 10").1;
    assert!(line.contains("Bloodmage's Regalia"), "{line}");
    // the game asks about it: the crate reads the answer
    w.raw(0x056, &50731u32.to_le_bytes());
    let (item, name) = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_ITEM_QUERY_SINGLE_RESPONSE(r)) => Some((r.item, r.found.as_ref().map(|x| x.name1.clone()))),
        _ => None,
    });
    assert_eq!((item, name.as_deref()), (50731, Some("Dojo Staff")));
    // saved, off (the weapons go), back on
    w.say(".den gearset save raid");
    assert_eq!(until_said(&mut w, "Saved").1, "Saved what you wear as 'raid'.");
    assert!(std::fs::read_to_string(dir.join(den_lite::gearsets::FILE)).unwrap().contains("\"raid\""));
    w.say(".den gearset off");
    let gone = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) => Some(d.guid.guid()),
        _ => None,
    });
    assert!(made.iter().any(|m| m.0 == gone), "a weapon item taken back");
    until_said(&mut w, "Back in your own outfit");
    w.say(".den gearset raid");
    let v = w.until(|f| values(f).filter(|(g, v)| *g == me && v.contains_key(&vis)).map(|(_, v)| v));
    assert_eq!(v[&vis], 51281);
    assert!(until_said(&mut w, "Your 'raid'").1.ends_with("set is on."));
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "gearset" && l.detail.starts_with("t10 10 ")));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The test range (owner 3 Nov: "better combat and test range dojo features"): a practice run (your bar cast for
/// you at a dummy) ends with a number; the boss casts at you and a kick stops it, then marks the ground; a pet comes
/// with its bar, answers to its name, and goes.
#[test]
fn a_practice_run_has_a_number_the_boss_is_kicked_and_a_pet_comes_and_goes() {
    let (srv, dir) = server("practice", HeroGender::Male);
    combat_tables(&dir);
    let mut w = into_world(&srv, 0);
    w.send(&heartbeat(16226.2, 16257.0, 13.25, 0.0));
    w.say(".den class mage fire 80");
    until_said(&mut w, "You are a level 80");
    w.say(".den dojo dummies 1");
    let (frames, _) = w.until_line();
    assert_eq!(unit_creates(&frames).len(), 1);
    w.say(".den dojo demo 5");
    until_said(&mut w, "A 5 second practice run");
    let mut casts = 0;
    let line = loop {
        let (frames, line) = w.until_line();
        casts += frames.iter().filter(|f| f.opcode == 0x0131).count();
        if line.starts_with("Practice run done") {
            break line;
        }
    };
    let dps: u32 = line.split(", ").nth(1).and_then(|x| x.split(' ').next()).and_then(|x| x.parse().ok()).unwrap_or(0);
    assert!(casts >= 1 && dps > 0, "{casts} casts: {line}");

    // the boss: one hit starts it; its cast at you, kicked
    w.say(".den dojo boss");
    let (frames, _) = w.until_line();
    let boss = unit_creates(&frames).into_iter().find(|u| u.1 == 900_005).expect("the boss").0;
    w.say(".den dojo cooldowns");
    w.until_line();
    w.raw(0x12E, &cast_at(5, 2136, boss));
    let spell_of = |b: &[u8], who: u64| -> Option<u32> {
        let mut at = 0;
        (den_lite::combat::read_packed(b, &mut at) == Some(who)).then_some(())?;
        den_lite::combat::read_packed(b, &mut at);
        Some(u32::from_le_bytes(b.get(at + 1..at + 5)?.try_into().ok()?))
    };
    w.until(|f| (f.opcode == 0x0131 && spell_of(&f.body, boss) == Some(den_lite::combat::BOSS_CAST_SPELL)).then_some(()));
    w.raw(0x12E, &cast_at(6, 2139, boss));
    let stopped = w.until(|f| (f.opcode == 0x0133).then(|| f.body.clone()));
    let mut at = 0;
    assert_eq!(den_lite::combat::read_packed(&stopped, &mut at), Some(boss), "the boss's cast stopped");
    w.until(|f| (f.opcode == 0x0131 && spell_of(&f.body, boss) == Some(den_lite::combat::BOSS_SPELL)).then_some(()));
    // the circle's warning comes with the boss's spell or just after it
    while !w.notes.iter().any(|n| n.starts_with("Fire at your feet")) {
        w.frame();
    }
    assert!(w.notes.iter().any(|n| n == "Interrupted!"), "{:?}", w.notes);

    // a pet: its create (read by the crate), its bar, its name, then dismissed from the bar
    w.raw(0x12E, &cast_at(7, 688, 0));
    let pet = w.until(|f| unit_creates(std::slice::from_ref(f)).into_iter().find(|u| u.1 == 416).map(|u| u.0));
    let bar = w.until(|f| (f.opcode == 0x0179).then(|| f.body.clone()));
    assert_eq!(u64::from_le_bytes(bar[..8].try_into().unwrap()), pet);
    w.send(&CMSG_CREATURE_QUERY { creature: 416, guid: Guid::new(pet) });
    let name = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_CREATURE_QUERY_RESPONSE(r)) if r.creature_entry == 416 => Some(r.found.as_ref().map(|x| x.name1.clone())),
        _ => None,
    });
    assert_eq!(name.as_deref(), Some("Imp"));
    let mut act = pet.to_le_bytes().to_vec();
    act.extend_from_slice(&(3u32 | 7 << 24).to_le_bytes());
    act.extend_from_slice(&0u64.to_le_bytes());
    w.raw(0x175, &act);
    let gone = w.until(|f| match &f.msg {
        Some(ServerOpcodeMessage::SMSG_DESTROY_OBJECT(d)) => Some(d.guid.guid()),
        _ => None,
    });
    assert_eq!(gone, pet);
    let evs = den_lite::events::read(&dir);
    assert!(evs.iter().any(|l| l.ev == "demo"));
    assert!(evs.iter().any(|l| l.ev == "dojo" && l.detail == "boss kicked"));
    assert!(evs.iter().any(|l| l.ev == "dojo" && l.detail == "boss circle"));
    assert!(evs.iter().any(|l| l.ev == "pet" && l.detail.starts_with("Imp")));
    drop(w);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dir);
}
