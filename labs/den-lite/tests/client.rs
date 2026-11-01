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
    let dir = std::env::temp_dir().join(format!("den-lite-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config {
        login_port: 0,
        world_port: 0,
        password: "den".into(),
        hero: Hero { name: "Wren".into(), gender },
        den_dir: dir.clone(),
        stamps_dir: stamps(),
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
        World { s, c }
    }

    fn frame(&mut self) -> Frame {
        let h = self.c.read_and_decrypt_server_header(&mut self.s).expect("a frame");
        let mut body = vec![0u8; h.size as usize - 2];
        self.s.read_exact(&mut body).unwrap();
        Frame { opcode: h.opcode, msg: reparse(h.opcode, &body), body }
    }

    fn send<M: ClientMessage>(&mut self, m: &M) {
        let mut v = Vec::new();
        m.write_encrypted_client(&mut v, self.c.encrypter()).unwrap();
        self.s.write_all(&v).unwrap();
    }

    fn say(&mut self, text: &str) {
        self.send(&CMSG_MESSAGECHAT { chat_type: CMSG_MESSAGECHAT_ChatType::Say, language: Language::Common, message: text.into() });
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
    // the quick start: four lines, the first one places a chair
    let tutorial: Vec<String> = (0..4).map(|_| w.until_line().1).collect();
    assert!(tutorial[0].contains(".den place chair") && tutorial[3].contains(".den save"), "{tutorial:?}");
    let names: Vec<u16> = frames.iter().map(|f| f.opcode).collect();
    let at = |op: u16| names.iter().position(|&o| o == op).unwrap_or_else(|| panic!("no {op:#x} in {names:x?}"));
    // SMSG_LOGIN_VERIFY_WORLD first, then account data, ..., the player, world states, the room
    assert_eq!(at(0x0236), 0);
    assert!(at(0x0209) < at(0x00A9), "account data times before the player create");
    assert!(at(0x0390) < at(0x00A9), "time sync before the player create");
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
    assert_eq!(player.0.unit_displayid(), Some(49));
    assert!(player.0.player_visible_item(VisibleItemIndex::Index3).is_some(), "the shirt shows");
    let Some(MovementBlock_UpdateFlag_Living::Living { position, running_speed, .. }) = player.1.update_flag.get_living() else {
        panic!("the player isn't living")
    };
    assert!((position.z - 14.2).abs() < 0.01 && *running_speed == 7.0);
    assert!(names.contains(&0x02C2), "SMSG_INIT_WORLD_STATES");
    let (room, _) = objects(&frames);
    assert_eq!(room.len(), 64 + 60 + 4, "floor, walls and lights");
    assert_eq!(room.values().filter(|g| g.entry == 188142 && g.display == 6752).count(), 124);
    assert_eq!(room.values().filter(|g| g.entry == 190664 && g.display == 7976).count(), 4);
    assert!(room.values().all(|g| g.scale == 2.0));
    // east and west walls are turned a quarter: their packed rotation carries z = sin(45 degrees)
    assert!(room.values().any(|g| (g.o - std::f32::consts::FRAC_PI_2).abs() < 1e-4 && g.rotation == (0.70710677f32 * (1 << 20) as f32) as u64));

    // walk 3 yards east, 2 north, facing east; then a stamp at our feet
    let (px, py, pz) = (16226.2 + 3.0, 16257.0 + 2.0, 14.2);
    w.send(&heartbeat(px, py, pz, 0.0));
    w.say(".den stamp campfire now");
    let (frames, line) = w.until_line();
    assert!(line.starts_with("Campfire circle is down (10 props)"), "{line}");
    let (stamp, _) = objects(&frames);
    assert_eq!(stamp.len(), 10);
    let fire = stamp.values().find(|g| g.entry == 1953).expect("the campfire");
    assert_eq!(fire.display, 192);
    assert!((fire.x - px).abs() < 0.01 && (fire.y - py).abs() < 0.01 && (fire.z - pz).abs() < 0.01);
    assert!(stamp.values().any(|g| g.entry == 24538 && (g.x - (px + 3.0)).abs() < 0.01 && (g.y - py).abs() < 0.01), "a bench 3 yards ahead");
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
    assert!(line.starts_with("Saved to a file (10 props)"), "{line}");
    let saved = den::read_file(&dir.join("test1.den.json")).expect("a Den file");
    assert_eq!(saved.props.len(), 10);
    let f = saved.props.iter().find(|p| p.entry == 1953).unwrap();
    assert!((f.x - 3.0).abs() < 0.01 && (f.y - 2.0).abs() < 0.01 && (f.z - 1.0).abs() < 0.01);
    assert_eq!(saved.room.as_ref().unwrap().look, "white");

    w.say(".build undo");
    let (frames, line) = w.until_line();
    let (_, gone) = objects(&frames);
    assert_eq!(gone.len(), 10, "{line}");
    assert!(stamp_guids.iter().all(|g| gone.contains(g)));

    w.say(".den load test1");
    let (frames, line) = w.until_line();
    assert_eq!(line, "Opened: 10 props.");
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
    assert_eq!((made.len(), gone.len()), (10, 10));
    assert!(dir.join("backup.den.json").exists(), "a copy is kept as backup");
    w.say(".build undo");
    let (frames, line) = w.until_line();
    let (made, gone) = objects(&frames);
    assert_eq!((made.len(), gone.len()), (10, 10), "{line}");
    assert_eq!(srv.shared.den.lock().unwrap().props.len(), 10);

    // a look, other questions, and out
    w.say(".den room stone");
    let (frames, line) = w.until_line();
    let (made, gone) = objects(&frames);
    assert_eq!(line, "The room is stone now.");
    assert_eq!(gone.len(), 128);
    assert!(made.values().all(|g| [190024, 19444, 180043].contains(&g.entry)));
    // the launcher's Saved Dens > Open, while the player is in: open.json (host/dens.rs open_doc)
    std::fs::write(dir.join("open.json"), r#"{"file":"test1","id":"18f3a9c2b7e1d4a51234"}"#).unwrap();
    let (_, line) = w.until_line();
    assert_eq!(line, "Opening 'test1' from the launcher.");
    let (frames, line) = w.until_line();
    assert_eq!(line, "Opened: 10 props.");
    assert_eq!(objects(&frames).1.len(), 10 + 98, "what was there goes, and the file's white room comes back for the stone one");
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
