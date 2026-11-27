//! Characters that survive any server (docs/UNSTOPPABLE.md phase 4, docs/leyline/characters.md): signed character
//! saves, sealed backups on friends, witness fingerprints and "Bring my characters" into another local world.
//!
//! * **The save** ([`save_doc`], [`make_record`], [`check_record`]): one character of a Built-in engine world read from
//!   its database ([`read_char`]) into canonical JSON. Things are named `<namespace>:<kind>:<entry>` (for example
//!   `stock-335a:item:19019`: [`NS`], [`name`]), never by their game names or files. Each save carries `seq`, `prev`
//!   (sha256 of the previous save's document), `world` (the realm id of the host's world key) and `made`. It is signed
//!   twice, Ed25519 over `leyline-char-save-v1 + "\n" + canonical(save)`: by the host's world key (`host_sig`) and by
//!   the player's key (`player_sig`). For the host's own characters both keys are on the same PC; for a friend's
//!   character on the host's world the host signs first and the friend's launcher countersigns ([`on_message`]
//!   `cosign`). [`check_chain`] checks the order of a line of saves; [`check_next`] refuses an old `seq` shown again.
//! * **When**: when a world stops ([`on_world_stop`], called from engine.rs `stop` while its database still runs), every
//!   30 minutes while it runs ([`start_timer`]) and on "Back up now". The last [`KEEP`] per character are kept in
//!   `<app data>/characters/<char id>/`. A save identical to the newest (but for its time) isn't made again.
//! * **Sealed backups on friends**: the newest save, deflated and sealed to the player's own key (identity.rs `seal`),
//!   sent in parts over the friends' sealed messages (kind `chars`) to up to 3 friends ([`share_newest`]). Friends keep
//!   others' blobs in `<app data>/characters/held/` (newest per owner and character, [`HELD_MAX`] in all) and hand
//!   them back when the owner's key asks (`ask` / `back`). They can't open them.
//! * **Witnesses**: `{char, seq, hash}` signed by the player (`leyline-char-witness-v1`), tiny, kept by any friend's
//!   launcher (`witnesses.json`), so the newest save can be proven and an older or forked one shows ([`judge`]).
//! * **Re-home** ([`customs`], [`import_sql`], [`bring`]): a verified save made into a character of another local
//!   world, under that world's rules: its level cap, items it doesn't know go to a vault list, gold never travels,
//!   raid lockouts and quests in progress stay behind, talents are picked again. Items arrive by in-game mail (no
//!   bag-slot guessing). Written in one transaction, while the worlds are stopped, after a back-up of that world.
//!
//! The keys and the friend transport are behind two small traits ([`Keys`], [`FriendLink`]). The player signs with the
//! Passport's world key for the save's world (`passport::world_identity`: a host sees only that key, as at login);
//! saves made with the root key before that stay the player's ([`my_player_key`]). Friends' messages go by
//! `friendnet` `deliver`: the same network, a shared direct address, then the connections by key from anywhere
//! (p2p/linknet.rs), then sealed and kept by a friend you both have until they are back.
//! No game data is ever copied or sent: only ids and numbers.

use crate::host::engine::{self as eng, Kind};
use crate::identity::{self, PeerIdentity};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const CTX_SAVE: &str = "leyline-char-save-v1";
pub const CTX_WITNESS: &str = "leyline-char-witness-v1";
/// The namespace of the Built-in engine's worlds: the stock data of the 3.3.5a client's era.
pub const NS: &str = "stock-335a";
/// Saves kept per character on this PC (the newest is always among them).
pub const KEEP: usize = 20;
/// A save while a world runs, this often.
pub const EVERY: u64 = 30 * 60;
pub const HOLDERS_DEFAULT: usize = 3;
pub const HOLDERS_MAX: usize = 8;
/// What this PC keeps of friends' sealed backups, in all.
pub const HELD_MAX: u64 = 20 * 1024 * 1024;
/// One sealed backup at most (base32 text).
pub const BLOB_MAX: usize = 1024 * 1024;
/// Base32 characters per message part (a friends' message is at most 8 KB).
pub const PART: usize = 5000;
pub const PARTS_MAX: usize = 220;
pub const WITNESS_MAX: usize = 2000;
/// A save's canonical form at most (a character with every quest and spell is far less).
pub const SAVE_MAX: usize = 512 * 1024;
const INCOMING_MAX: usize = 64;
const INCOMING_TTL: u64 = 24 * 3600;
/// Items one re-home carries at most (the rest go to the vault).
const ITEMS_MAX: usize = 300;
const MAIL_ITEMS: usize = 12;

// ------------------------------------------------------------------------------------------------ small helpers

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn sha_hex(b: &[u8]) -> String {
    identity::hex(&Sha256::digest(b))
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `<ns>:<kind>:<entry>`.
pub fn name(kind: &str, entry: u64) -> String {
    format!("{NS}:{kind}:{entry}")
}

/// The entry of a name of this kind in namespace `ns`.
pub fn entry_of(s: &str, ns: &str, kind: &str) -> Option<u64> {
    let mut it = s.splitn(3, ':');
    let (n, k, e) = (it.next()?, it.next()?, it.next()?);
    (n == ns && k == kind).then(|| e.parse().ok()).flatten()
}

/// A character's id: made once, when its first save is made, from the world it was born in and its row there; kept
/// for good (a re-homed character keeps it).
pub fn char_id(realm_id: &str, guid: u64) -> String {
    let d = Sha256::digest(format!("leyline-char-v1\n{realm_id}\n{guid}").as_bytes());
    format!("c{}", &identity::b32(&d)[..15]).to_ascii_lowercase()
}

pub fn char_id_ok(s: &str) -> bool {
    s.len() == 16 && s.starts_with('c') && s.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

fn deflate(b: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(b);
    e.finish().unwrap_or_default()
}

fn inflate(b: &[u8], max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(b).take(max as u64 + 1).read_to_end(&mut out).map_err(|_| "not a save".to_string())?;
    if out.len() > max {
        return Err("too big".into());
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ the two traits

/// The keys a save is signed with. Today: the player's peer key (identity.rs) and each world's realm key (litenet.rs,
/// `<world>/share/realm.key`). The Passport (feat-passport) replaces [`Keys::player`] with its own key.
pub trait Keys {
    /// The player's key for the world with this realm id (the Passport's world key).
    fn player(&self, realm: &str) -> Result<PeerIdentity, String>;
    fn world(&self, kind: Kind) -> Result<PeerIdentity, String>;
}

/// The key that signs as the player of `rec` on this PC (`root`: the Passport): the world key for the save's world, or
/// the root itself for a save made before world keys. None: not this player's save.
pub fn my_player_key(root: &PeerIdentity, rec: &Value) -> Option<PeerIdentity> {
    let p = rec["save"]["player"].as_str()?;
    if p == root.public_hex() {
        return Some(root.duplicate());
    }
    let w = crate::passport::world_identity(root, rec["save"]["world"].as_str()?);
    (w.public_hex() == p).then_some(w)
}

/// Sending a small signed body to a friend, and who the friends are: friendnet's sealed messages (kind `chars`),
/// which go the same network, direct, by key from anywhere (p2p/linknet.rs) or wait sealed with a shared friend.
pub trait FriendLink {
    /// Each friend: (public key hex, name, last seen).
    fn friends(&self) -> Vec<(String, String, u64)>;
    fn send(&self, to: &str, body: Value) -> Result<(), String>;
}

/// This PC's keys.
pub struct LocalKeys {
    pub app: PathBuf,
}

impl Keys for LocalKeys {
    fn player(&self, realm: &str) -> Result<PeerIdentity, String> {
        Ok(crate::passport::world_identity(&identity::load_or_create(&self.app)?, realm))
    }
    fn world(&self, kind: Kind) -> Result<PeerIdentity, String> {
        use crate::host::{litenet, worldshare as ws};
        let share = ws::dir(&eng::Paths::for_kind(&self.app, kind).root);
        std::fs::create_dir_all(&share).map_err(|e| e.to_string())?;
        PeerIdentity::from_seed(litenet::load_or_create_key(&litenet::key_path(&share))?)
    }
}

/// The friends' network running in this launcher (p2p/friendnet.rs) as a [`FriendLink`].
pub struct HubLink<'a>(pub &'a crate::p2p::friendnet::Hub);

impl FriendLink for HubLink<'_> {
    fn friends(&self) -> Vec<(String, String, u64)> {
        let mine = self.0.me.public_hex();
        let b = self.0.book.lock().unwrap_or_else(|e| e.into_inner());
        b.friends
            .iter()
            .filter_map(|f| {
                let d = &f.friendship["doc"];
                let (a, c) = (d["a"].as_str()?, d["b"].as_str()?);
                let other = if a == mine { c } else { a };
                let name = f.card.as_ref().and_then(|c| c["card"]["name"].as_str()).unwrap_or("").to_string();
                Some((other.to_string(), name, f.last_seen))
            })
            .collect()
    }
    fn send(&self, to: &str, body: Value) -> Result<(), String> {
        let m = crate::p2p::friends::make_msg(&self.0.me, to, "chars", body, now())?;
        self.0.deliver(to, &m, None).map(|_| ())
    }
}

// ------------------------------------------------------------------------------------------------ reading a character

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    pub entry: u64,
    pub count: u64,
    /// (enchantment slot, enchantment id), the lasting ones only (no temporary ones).
    pub enchants: Vec<(u64, u64)>,
    pub suffix: i64,
    pub place: &'static str,
}

/// One character as its world's database has it (numbers only).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharState {
    pub guid: u64,
    pub account: String,
    pub name: String,
    pub race: u64,
    pub class: u64,
    pub female: bool,
    pub level: u64,
    pub xp: u64,
    pub money: u64,
    pub played: u64,
    /// skin, face, hair style, hair colour, facial hair.
    pub looks: [u64; 5],
    pub items: Vec<Item>,
    pub done: Vec<u64>,
    pub active: Vec<u64>,
    /// (faction, standing, flags)
    pub rep: Vec<(u64, i64, u64)>,
    /// (skill, value, max)
    pub skills: Vec<(u64, u64, u64)>,
    pub spells: Vec<u64>,
    /// (achievement, date)
    pub achievements: Vec<(u64, u64)>,
    /// (map, difficulty, reset time)
    pub raids: Vec<(u64, u64, u64)>,
}

/// The player's own characters' rows in a world (never deleted ones).
pub fn guids_sql(accounts: &[String]) -> Result<String, String> {
    if accounts.is_empty() || !accounts.iter().all(|a| (3..=16).contains(&a.len()) && a.bytes().all(|b| b.is_ascii_alphanumeric())) {
        return Err("That login doesn't look right.".into());
    }
    let list: Vec<String> = accounts.iter().map(|a| format!("'{}'", a.to_ascii_uppercase())).collect();
    Ok(format!(
        "SELECT c.guid FROM characters c JOIN acore_auth.account a ON a.id = c.account WHERE a.username IN ({}) AND c.deleteInfos_Name IS NULL ORDER BY c.guid",
        list.join(", ")
    ))
}

pub fn char_sql(guid: u64) -> String {
    format!(
        "SELECT c.name, c.race, c.class, c.gender, c.level, c.xp, c.money, c.totaltime, c.skin, c.face, c.hairStyle, c.hairColor, c.facialStyle, a.username \
         FROM characters c JOIN acore_auth.account a ON a.id = c.account WHERE c.guid = {guid} AND c.deleteInfos_Name IS NULL"
    )
}

/// Everything else of one character in one query: tagged rows of 7 columns.
pub fn lists_sql(guid: u64) -> String {
    let g = guid;
    [
        format!("SELECT 'i', ii.itemEntry, ii.count, ii.randomPropertyId, ci.bag, ci.slot, ii.enchantments FROM character_inventory ci JOIN item_instance ii ON ii.guid = ci.item WHERE ci.guid = {g}"),
        format!("SELECT 'q', quest, 0, 0, 0, 0, '' FROM character_queststatus_rewarded WHERE guid = {g}"),
        format!("SELECT 'a', quest, status, 0, 0, 0, '' FROM character_queststatus WHERE guid = {g}"),
        format!("SELECT 'r', faction, standing, flags, 0, 0, '' FROM character_reputation WHERE guid = {g}"),
        format!("SELECT 's', skill, value, max, 0, 0, '' FROM character_skills WHERE guid = {g}"),
        format!("SELECT 'p', spell, 0, 0, 0, 0, '' FROM character_spell WHERE guid = {g}"),
        format!("SELECT 'h', achievement, date, 0, 0, 0, '' FROM character_achievement WHERE guid = {g}"),
        format!("SELECT 'd', i.map, i.difficulty, i.resettime, 0, 0, '' FROM character_instance ci JOIN instance i ON i.id = ci.instance WHERE ci.guid = {g} AND ci.permanent = 1"),
    ]
    .join(" UNION ALL ")
}

/// Where an inventory row is: worn, in the bags, in the bank, on the key ring.
pub fn place(bag: u64, slot: u64) -> &'static str {
    if bag != 0 {
        return "bags";
    }
    match slot {
        0..=18 => "worn",
        19..=38 => "bags",
        39..=73 => "bank",
        86..=117 => "keys",
        _ => "other",
    }
}

/// The lasting enchantments of an `enchantments` column (12 slots of id, duration, charges). The temporary slot (1)
/// is left out.
pub fn parse_enchants(text: &str) -> Vec<(u64, u64)> {
    let n: Vec<u64> = text.split_whitespace().map(|x| x.parse().unwrap_or(0)).collect();
    (0..12u64).filter(|i| *i != 1).filter_map(|i| n.get(i as usize * 3).copied().filter(|id| *id != 0).map(|id| (i, id))).collect()
}

/// One character from its world's database (`q`: one query's `-N -B` rows, engine.rs `query`).
pub fn read_char(q: &dyn Fn(&str) -> Result<String, String>, guid: u64) -> Result<CharState, String> {
    let row = q(&char_sql(guid))?;
    let f: Vec<&str> = row.lines().next().ok_or("That character isn't there.")?.trim_end_matches('\r').split('\t').collect();
    if f.len() < 14 || !crate::hybrid::char_name_ok(f[0]) {
        return Err("That character's row doesn't look right.".into());
    }
    let n = |i: usize| f[i].trim().parse::<u64>().unwrap_or(0);
    let mut st = CharState {
        guid,
        account: f[13].trim().to_string(),
        name: f[0].to_string(),
        race: n(1),
        class: n(2),
        female: n(3) == 1,
        level: n(4),
        xp: n(5),
        money: n(6),
        played: n(7),
        looks: [n(8), n(9), n(10), n(11), n(12)],
        ..Default::default()
    };
    for line in q(&lists_sql(guid))?.lines() {
        let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
        if f.len() < 7 {
            continue;
        }
        let u = |i: usize| f[i].trim().parse::<u64>().ok();
        let i = |i: usize| f[i].trim().parse::<i64>().ok();
        match (f[0], u(1)) {
            ("i", Some(e)) if e > 0 => st.items.push(Item {
                entry: e,
                count: u(2).unwrap_or(1).max(1),
                suffix: i(3).unwrap_or(0),
                place: place(u(4).unwrap_or(0), u(5).unwrap_or(0)),
                enchants: parse_enchants(f[6]),
            }),
            ("q", Some(qid)) => st.done.push(qid),
            ("a", Some(qid)) => st.active.push(qid),
            ("r", Some(fa)) => st.rep.push((fa, i(2).unwrap_or(0), u(3).unwrap_or(0))),
            ("s", Some(s)) => st.skills.push((s, u(2).unwrap_or(0), u(3).unwrap_or(0))),
            ("p", Some(s)) => st.spells.push(s),
            ("h", Some(a)) => st.achievements.push((a, u(2).unwrap_or(0))),
            ("d", Some(m)) => st.raids.push((m, u(2).unwrap_or(0), u(3).unwrap_or(0))),
            _ => {}
        }
    }
    st.done.sort_unstable();
    st.done.dedup();
    st.active.sort_unstable();
    st.spells.sort_unstable();
    st.spells.dedup();
    st.rep.sort_unstable();
    st.skills.sort_unstable();
    st.achievements.sort_unstable();
    st.raids.sort_unstable();
    st.items.sort_by(|a, b| (a.place, a.entry, a.count).cmp(&(b.place, b.entry, b.count)));
    Ok(st)
}

// ------------------------------------------------------------------------------------------------ the save record

/// What a save says about the character (no chain fields): the part compared to tell whether anything changed.
pub fn content(st: &CharState) -> Value {
    let items: Vec<Value> = st
        .items
        .iter()
        .map(|it| {
            let mut o = json!({"id": name("item", it.entry), "n": it.count, "at": it.place});
            if !it.enchants.is_empty() {
                o["ench"] = json!(it.enchants.iter().map(|(s, e)| json!([s, name("enchant", *e)])).collect::<Vec<_>>());
            }
            if it.suffix != 0 {
                o["suffix"] = json!(it.suffix);
            }
            o
        })
        .collect();
    json!({
        "name": st.name,
        "race": name("race", st.race),
        "class": name("class", st.class),
        "gender": if st.female { "female" } else { "male" },
        "level": st.level,
        "xp": st.xp,
        "money": st.money,
        "played": st.played,
        "looks": st.looks.to_vec(),
        "items": items,
        "quests": {"done": st.done.iter().map(|q| name("quest", *q)).collect::<Vec<_>>(),
                   "active": st.active.iter().map(|q| name("quest", *q)).collect::<Vec<_>>()},
        "rep": st.rep.iter().map(|(f, s, fl)| json!({"id": name("faction", *f), "standing": s, "flags": fl})).collect::<Vec<_>>(),
        "skills": st.skills.iter().map(|(s, v, m)| json!({"id": name("skill", *s), "value": v, "max": m})).collect::<Vec<_>>(),
        "spells": st.spells.iter().map(|s| name("spell", *s)).collect::<Vec<_>>(),
        "achievements": st.achievements.iter().map(|(a, d)| json!({"id": name("achievement", *a), "date": d})).collect::<Vec<_>>(),
        "raids": st.raids.iter().map(|(m, d, r)| json!({"map": name("map", *m), "difficulty": d, "reset": r})).collect::<Vec<_>>(),
    })
}

pub fn hash_doc(doc: &Value) -> String {
    sha_hex(identity::canonical(doc).as_bytes())
}

/// The save document: `prev` the record before it (None: the first).
pub fn save_doc(st: &CharState, prev: Option<&Value>, char: &str, world: &str, player: &str, host: &str, made: u64) -> Value {
    let (seq, prev_hash) = match prev {
        Some(p) => (p["save"]["seq"].as_u64().unwrap_or(0) + 1, hash_doc(&p["save"])),
        None => (1, String::new()),
    };
    json!({"v": 1, "char": char, "seq": seq, "prev": prev_hash, "world": world, "made": made, "ns": NS,
           "player": player, "host": host, "who": content(st)})
}

pub fn make_record(doc: Value, host: Option<&PeerIdentity>, player: Option<&PeerIdentity>) -> Value {
    let mut r = json!({"kind": "char-save", "save": doc, "host_sig": "", "player_sig": ""});
    if let Some(h) = host {
        r["host_sig"] = json!(h.sign_doc(CTX_SAVE, &r["save"]));
    }
    if let Some(p) = player {
        r["player_sig"] = json!(p.sign_doc(CTX_SAVE, &r["save"]));
    }
    r
}

/// The shape of a save and its host's signature (the half a host makes before the player countersigns).
pub fn check_half(rec: &Value) -> Result<(), String> {
    if rec["kind"] != "char-save" {
        return Err("not a character save".into());
    }
    let s = &rec["save"];
    if identity::canonical(s).len() > SAVE_MAX {
        return Err("save too big".into());
    }
    if s["v"].as_u64() != Some(1) {
        return Err("a newer kind of save".into());
    }
    let char = s["char"].as_str().unwrap_or("");
    let seq = s["seq"].as_u64().unwrap_or(0);
    let prev = s["prev"].as_str().unwrap_or("-");
    let (player, host) = (s["player"].as_str().unwrap_or(""), s["host"].as_str().unwrap_or(""));
    if !char_id_ok(char) || seq == 0 || !(if seq == 1 { prev.is_empty() } else { hex64(prev) }) {
        return Err("save fields".into());
    }
    if !hex64(player) || !hex64(host) || s["made"].as_u64().is_none() || !s["who"].is_object() || s["ns"].as_str().map_or(true, |n| n.is_empty()) {
        return Err("save fields".into());
    }
    let hk = identity::unhex32(host).ok_or("host key")?;
    if s["world"].as_str() != Some(identity::peer_id(&hk).as_str()) {
        return Err("the world isn't the host key's".into());
    }
    if !identity::verify_doc(host, s, CTX_SAVE, rec["host_sig"].as_str().unwrap_or("")) {
        return Err("the host's signature doesn't match".into());
    }
    Ok(())
}

/// A whole save: its shape and both signatures.
pub fn check_record(rec: &Value) -> Result<(), String> {
    check_half(rec)?;
    let s = &rec["save"];
    if !identity::verify_doc(s["player"].as_str().unwrap_or(""), s, CTX_SAVE, rec["player_sig"].as_str().unwrap_or("")) {
        return Err("the player's signature doesn't match".into());
    }
    Ok(())
}

/// A line of saves, oldest first: each valid, one character and one player, `seq` one more each time and `prev` the
/// one before it.
pub fn check_chain(recs: &[Value]) -> Result<(), String> {
    let first = recs.first().ok_or("no saves")?;
    for (i, r) in recs.iter().enumerate() {
        check_record(r).map_err(|e| format!("save {}: {e}", i + 1))?;
        if r["save"]["char"] != first["save"]["char"] || r["save"]["player"] != first["save"]["player"] {
            return Err(format!("save {}: another character's", i + 1));
        }
        if i > 0 {
            let p = &recs[i - 1]["save"];
            if r["save"]["seq"].as_u64() != p["seq"].as_u64().map(|x| x + 1) {
                return Err(format!("save {}: out of order", i + 1));
            }
            if r["save"]["prev"].as_str() != Some(hash_doc(p).as_str()) {
                return Err(format!("save {}: doesn't follow the one before", i + 1));
            }
        }
    }
    Ok(())
}

/// Whether `rec` may follow the newest save known (`known`: its seq and document hash). An older seq, or the same
/// seq with other contents, is refused (a replay or a fork); seq+1 must point at the newest; a gap is taken (saves
/// in between were lost) only when `gaps` allows.
pub fn check_next(known: Option<(u64, &str)>, rec: &Value, gaps: bool) -> Result<bool, String> {
    let seq = rec["save"]["seq"].as_u64().unwrap_or(0);
    let Some((kseq, khash)) = known else { return Ok(true) };
    let h = hash_doc(&rec["save"]);
    if seq < kseq {
        return Err(format!("an old save (number {seq}; the newest is {kseq})"));
    }
    if seq == kseq {
        return if h == khash { Ok(false) } else { Err("a different save with the same number (a fork)".into()) };
    }
    if seq == kseq + 1 && rec["save"]["prev"].as_str() != Some(khash) {
        return Err("a save that doesn't follow the newest (a fork)".into());
    }
    if seq > kseq + 1 && !gaps {
        return Err("saves are missing in between".into());
    }
    Ok(true)
}

// ------------------------------------------------------------------------------------------------ witnesses

pub fn make_witness(player: &PeerIdentity, rec: &Value, made: u64) -> Value {
    let w = json!({"v": 1, "char": rec["save"]["char"], "seq": rec["save"]["seq"], "hash": hash_doc(&rec["save"]),
                   "player": player.public_hex(), "made": made});
    let sig = player.sign_doc(CTX_WITNESS, &w);
    json!({"kind": "char-witness", "witness": w, "sig": sig})
}

pub fn check_witness(rec: &Value) -> Result<(), String> {
    let w = &rec["witness"];
    if rec["kind"] != "char-witness" || w["v"].as_u64() != Some(1) || identity::canonical(rec).len() > 1024 {
        return Err("not a witness".into());
    }
    if !char_id_ok(w["char"].as_str().unwrap_or("")) || w["seq"].as_u64().unwrap_or(0) == 0 || !hex64(w["hash"].as_str().unwrap_or("")) {
        return Err("witness fields".into());
    }
    if !identity::verify_doc(w["player"].as_str().unwrap_or(""), w, CTX_WITNESS, rec["sig"].as_str().unwrap_or("")) {
        return Err("the witness's signature doesn't match".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The save is the one the witness names.
    Newest,
    /// A newer save than the witness knows.
    Ahead,
    /// An older save than the newest one witnessed.
    Stale,
    /// The same number, other contents.
    Fork,
    /// Not the same character or player.
    Other,
}

/// What a witness says of a save.
pub fn judge(witness: &Value, rec: &Value) -> Verdict {
    let (w, s) = (&witness["witness"], &rec["save"]);
    if w["char"] != s["char"] || w["player"] != s["player"] {
        return Verdict::Other;
    }
    let (ws, ss) = (w["seq"].as_u64().unwrap_or(0), s["seq"].as_u64().unwrap_or(0));
    match ss.cmp(&ws) {
        std::cmp::Ordering::Less => Verdict::Stale,
        std::cmp::Ordering::Greater => Verdict::Ahead,
        std::cmp::Ordering::Equal if w["hash"].as_str() == Some(hash_doc(s).as_str()) => Verdict::Newest,
        _ => Verdict::Fork,
    }
}

// ------------------------------------------------------------------------------------------------ the store on this PC

pub fn dir(app: &Path) -> PathBuf {
    app.join("characters")
}

fn char_dir(app: &Path, id: &str) -> PathBuf {
    dir(app).join(id)
}

/// A friend holding a copy of a character's newest save.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Holder {
    pub key: String,
    pub name: String,
    pub seq: u64,
    pub at: u64,
}

/// A character this PC keeps saves of.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// The local world it lives in now ("single", "vanilla", "bossrush"; "" for one brought back from a friend).
    pub world: String,
    pub realm: String,
    pub guid: u64,
    /// The player's own (false: a friend's character on this PC's world).
    pub mine: bool,
    pub seq: u64,
    pub made: u64,
    pub holders: Vec<Holder>,
    /// Earlier homes: "<realm>:<guid>" (a re-homed character's old rows).
    pub homes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// How many friends get a sealed copy (0: none).
    pub holders: usize,
    /// Friends picked first (public keys hex).
    pub picked: Vec<String>,
    /// Keep friends' sealed copies on this PC.
    pub keep_for_friends: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { holders: HOLDERS_DEFAULT, picked: Vec::new(), keep_for_friends: true }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    pub chars: Vec<Entry>,
    pub settings: Settings,
    /// Items that didn't come along on a re-home: (char id, item name, count, why, when).
    pub vault: Vec<VaultItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultItem {
    pub char: String,
    pub id: String,
    pub n: u64,
    pub why: String,
    pub at: u64,
}

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn load_index(app: &Path) -> Index {
    std::fs::read_to_string(dir(app).join("index.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_index(app: &Path, i: &Index) -> Result<(), String> {
    std::fs::create_dir_all(dir(app)).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&dir(app).join("index.json"), serde_json::to_string_pretty(i).unwrap_or_default().as_bytes()).map_err(|e| e.to_string())
}

fn write_json(file: &Path, v: &Value) -> Result<(), String> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(file, identity::canonical(v).as_bytes()).map_err(|e| e.to_string())
}

fn read_json(file: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// A character's saves on this PC, oldest first (only ones that check out).
pub fn saves(app: &Path, id: &str) -> Vec<Value> {
    if !char_id_ok(id) {
        return Vec::new();
    }
    let mut names: Vec<PathBuf> = std::fs::read_dir(char_dir(app, id))
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.len() == 13 && n.ends_with(".json") && n[..8].bytes().all(|b| b.is_ascii_digit()))).collect())
        .unwrap_or_default();
    names.sort();
    names.iter().filter_map(|p| read_json(p)).filter(|r| check_record(r).is_ok() && r["save"]["char"] == id).collect()
}

pub fn newest(app: &Path, id: &str) -> Option<Value> {
    saves(app, id).pop()
}

/// Keep a checked save: after the newest (never an older or forked one), the last [`KEEP`] kept. True when new.
pub fn put_save(app: &Path, rec: &Value, gaps: bool) -> Result<bool, String> {
    check_record(rec)?;
    let id = rec["save"]["char"].as_str().unwrap_or("").to_string();
    let last = newest(app, &id);
    let known = last.as_ref().map(|l| (l["save"]["seq"].as_u64().unwrap_or(0), hash_doc(&l["save"])));
    if let Some(l) = &last {
        if l["save"]["player"] != rec["save"]["player"] {
            return Err("a save of this character by someone else".into());
        }
    }
    if !check_next(known.as_ref().map(|(s, h)| (*s, h.as_str())), rec, gaps)? {
        return Ok(false);
    }
    let seq = rec["save"]["seq"].as_u64().unwrap_or(0);
    write_json(&char_dir(app, &id).join(format!("{seq:08}.json")), rec)?;
    let mut all: Vec<PathBuf> = std::fs::read_dir(char_dir(app, &id)).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json") && p.file_stem().is_some_and(|s| s.len() == 8)).collect()).unwrap_or_default();
    all.sort();
    if all.len() > KEEP {
        for old in &all[..all.len() - KEEP] {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(true)
}

/// The index entry for a character's row in a world: by its home (the realm and row), else a new id.
fn entry_for<'a>(idx: &'a mut Index, realm: &str, guid: u64, mine: bool) -> &'a mut Entry {
    let home = format!("{realm}:{guid}");
    let pos = idx.chars.iter().position(|e| e.realm == realm && e.guid == guid && e.mine == mine)
        .or_else(|| idx.chars.iter().position(|e| e.homes.contains(&home) && e.mine == mine && e.realm == realm));
    match pos {
        Some(i) => &mut idx.chars[i],
        None => {
            idx.chars.push(Entry { id: char_id(realm, guid), realm: realm.into(), guid, mine, ..Default::default() });
            idx.chars.last_mut().unwrap()
        }
    }
}

// ------------------------------------------------------------------------------------------------ making saves

/// Saves for the given rows of one world (both keys here): one per character whose contents changed since its
/// newest. The ids saved.
pub fn make_saves_with(app: &Path, kind: Kind, q: &dyn Fn(&str) -> Result<String, String>, guids: &[u64], keys: &dyn Keys, made: u64) -> Result<Vec<String>, String> {
    let host = keys.world(kind)?;
    let realm = identity::peer_id(&host.public_key);
    let player = keys.player(&realm)?;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = load_index(app);
    let mut out = Vec::new();
    for g in guids {
        let st = match read_char(q, *g) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let e = entry_for(&mut idx, &realm, *g, true);
        let id = e.id.clone();
        let prev = newest(app, &id);
        if prev.as_ref().is_some_and(|p| p["save"]["who"] == content(&st) && p["save"]["world"] == realm.as_str()) {
            e.name = st.name.clone();
            e.world = kind.word().into();
            continue;
        }
        let doc = save_doc(&st, prev.as_ref(), &id, &realm, &player.public_hex(), &host.public_hex(), made);
        let rec = make_record(doc, Some(&host), Some(&player));
        if put_save(app, &rec, false)? {
            e.name = st.name.clone();
            e.world = kind.word().into();
            e.seq = rec["save"]["seq"].as_u64().unwrap_or(0);
            e.made = made;
            out.push(id);
        }
    }
    save_index(app, &idx)?;
    Ok(out)
}

/// The player's own logins in a world (its own, the Hero Key ones; Boss Rush: the hero account).
fn own_accounts(p: &eng::Paths) -> Vec<String> {
    let s = eng::load_state(p);
    let mut v = eng::char_accounts(p, &s);
    for u in eng::own_logins(&s) {
        if !v.iter().any(|x| x.eq_ignore_ascii_case(&u)) {
            v.push(u);
        }
    }
    v
}

/// The player's characters in a running world, saved now. How many saves were made.
pub fn save_world(p: &eng::Paths) -> Result<Vec<String>, String> {
    let s = eng::load_state(p);
    if !s.account_made || eng::other_running(p).is_some() || !eng::port_answers(eng::DB_PORT) {
        return Err(format!("{} isn't running.", p.kind.title()));
    }
    let q = |t: &str| eng::query(p, "acore_characters", t);
    let guids: Vec<u64> = q(&guids_sql(&own_accounts(p))?)?.lines().filter_map(|l| l.trim().parse().ok()).collect();
    make_saves_with(&p.app, p.kind, &q, &guids, &LocalKeys { app: p.app.clone() }, now())
}

/// engine.rs `stop`: the world's characters saved while its database still runs, then sent to friends (in the
/// background: stopping isn't held up). Errors only go to the world's setup log.
pub fn on_world_stop(p: &eng::Paths) {
    match save_world(p) {
        Ok(ids) => {
            if !ids.is_empty() {
                eng::setup_log(p, &format!("{} character saves made", ids.len()));
            }
            host_friend_saves(p);
            share_in_background(p.app.clone(), ids);
        }
        Err(e) => eng::setup_log(p, &format!("character saves weren't made: {e}")),
    }
}

/// "Back up now": the world must be running (its characters are read from it); else the newest saves are sent again.
pub fn back_up_now(app: &Path, kind: Kind) -> Result<String, String> {
    let p = eng::Paths::for_kind(app, kind);
    let ran = save_world(&p);
    let ids: Vec<String> = match &ran {
        Ok(_) => load_index(app).chars.iter().filter(|e| e.mine && e.world == kind.word()).map(|e| e.id.clone()).collect(),
        Err(_) => load_index(app).chars.iter().filter(|e| e.mine && e.world == kind.word()).map(|e| e.id.clone()).collect(),
    };
    if ran.is_err() && ids.is_empty() {
        return Err(format!("Start {} once (press Play) so its characters can be read, then back up.", kind.title()));
    }
    let sent = match crate::p2p::friendnet::current() {
        Some(h) => ids.iter().map(|id| share_newest(app, &HubLink(&h), id, now()).unwrap_or(0)).max().unwrap_or(0),
        None => 0,
    };
    let made = ran.as_ref().map(|v| v.len()).unwrap_or(0);
    let mut t = match (&ran, made) {
        (Err(_), _) => "Your newest saves were kept as they were (the world isn't running).".to_string(),
        (Ok(_), 0) => "Saved: nothing changed since the last save.".to_string(),
        (Ok(_), 1) => "Saved one character.".to_string(),
        (Ok(_), n) => format!("Saved {n} characters."),
    };
    t.push_str(&match sent {
        0 if crate::p2p::friendnet::current().is_none() => " Friends get a copy when your friends list is on.".to_string(),
        0 => " No friend could take a copy right now.".to_string(),
        1 => " One friend has a sealed copy.".to_string(),
        n => format!(" {n} friends have a sealed copy."),
    });
    Ok(t)
}

fn share_in_background(app: PathBuf, ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    if let Some(h) = crate::p2p::friendnet::current() {
        std::thread::spawn(move || {
            for id in ids {
                let _ = share_newest(&app, &HubLink(&h), &id, now());
            }
        });
    }
}

static LAST: std::sync::Mutex<u64> = std::sync::Mutex::new(0);

/// Whether the 30-minute save is due.
pub fn due(last: u64, now: u64) -> bool {
    now.saturating_sub(last) >= EVERY
}

/// Every 30 minutes, the running world's characters saved (and sent to friends). Started once by the app.
pub fn start_timer(app: PathBuf) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = now();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let Some(k) = eng::active() else { continue };
        let t = now();
        if !due(*LAST.lock().unwrap_or_else(|e| e.into_inner()), t) {
            continue;
        }
        *LAST.lock().unwrap_or_else(|e| e.into_inner()) = t;
        let p = eng::Paths::for_kind(&app, k);
        if eng::port_answers(eng::WORLD_PORT) {
            if let Ok(ids) = save_world(&p) {
                host_friend_saves(&p);
                share_in_background(app.clone(), ids);
            }
        }
    });
}

// ------------------------------------------------------------------------------------------------ friends' characters on my world

/// The host's half of a save for a friend's character on its world (the friend countersigns): host-signed, the
/// friend's key as the player. Kept as `<char>/pending.json` until the countersignature comes back.
pub fn half_save(app: &Path, kind: Kind, st: &CharState, friend_key: &str, keys: &dyn Keys, made: u64) -> Result<Option<Value>, String> {
    if !hex64(friend_key) {
        return Err("friend key".into());
    }
    let host = keys.world(kind)?;
    let realm = identity::peer_id(&host.public_key);
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = load_index(app);
    let e = entry_for(&mut idx, &realm, st.guid, false);
    let id = e.id.clone();
    e.name = st.name.clone();
    e.world = kind.word().into();
    let prev = newest(app, &id);
    if prev.as_ref().is_some_and(|p| p["save"]["who"] == content(st)) {
        save_index(app, &idx)?;
        return Ok(None);
    }
    let doc = save_doc(st, prev.as_ref(), &id, &realm, friend_key, &host.public_hex(), made);
    let rec = make_record(doc, Some(&host), None);
    write_json(&char_dir(app, &id).join("pending.json"), &rec)?;
    save_index(app, &idx)?;
    Ok(Some(rec))
}

/// Half saves for friends' characters on a running world, sent to each friend to countersign.
fn host_friend_saves(p: &eng::Paths) {
    use crate::host::worldshare as ws;
    let Some(h) = crate::p2p::friendnet::current() else { return };
    let book = crate::host::worldfriends::load(&ws::dir(&p.root));
    let Ok(rows) = eng::query(p, "acore_characters", &format!(
        "SELECT c.guid, a.username FROM characters c JOIN acore_auth.account a ON a.id = c.account WHERE a.username REGEXP '{}' AND c.deleteInfos_Name IS NULL",
        crate::host::worldfriends::FRIEND_REGEXP)) else { return };
    let q = |t: &str| eng::query(p, "acore_characters", t);
    let keys = LocalKeys { app: p.app.clone() };
    let link = HubLink(&h);
    let reachable: Vec<String> = link.friends().into_iter().map(|f| f.0).collect();
    for line in rows.lines() {
        let f: Vec<&str> = line.trim().split('\t').collect();
        let (Some(g), Some(u)) = (f.first().and_then(|g| g.parse::<u64>().ok()), f.get(1)) else { continue };
        // only a friend whose world login is their Friends key can be asked to countersign: a Passport world key
        // (a v2 login) can't be reached by its key, by design, so that friend's save stays host-signed (pending)
        let Some(fr) = book.friends.iter().find(|x| x.user == *u && hex64(&x.key)) else { continue };
        let Ok(st) = read_char(&q, g) else { continue };
        if let Ok(Some(rec)) = half_save(&p.app, p.kind, &st, &fr.key, &keys, now()) {
            if !reachable.contains(&fr.key) {
                continue;
            }
            let _ = send_parts(&link, &fr.key, "cosign", &rec["save"]["char"].as_str().unwrap_or("").to_string(),
                rec["save"]["seq"].as_u64().unwrap_or(0), &identity::b32(&deflate(identity::canonical(&rec).as_bytes())));
        }
    }
}

// ------------------------------------------------------------------------------------------------ sealed backups

/// The newest save sealed to its own player's Passport root (`owner`; not the world key, so the recovery phrase alone
/// opens it on a new PC, without knowing which worlds it played on): base32 of `seal(owner, deflate(canonical(record)))`.
pub fn seal_save(rec: &Value, owner: &[u8; 32]) -> Result<String, String> {
    let pk = *owner;
    let b = identity::b32(&identity::seal(&pk, &deflate(identity::canonical(rec).as_bytes()))?);
    if b.len() > BLOB_MAX {
        return Err("That save is too big to send.".into());
    }
    Ok(b)
}

/// Open a sealed backup with the player's key: the save, checked, and the player's.
pub fn open_save(me: &PeerIdentity, blob: &str) -> Result<Value, String> {
    let b = identity::unb32(blob).ok_or("not a backup")?;
    let rec: Value = serde_json::from_slice(&inflate(&me.open(&b)?, SAVE_MAX + 4096)?).map_err(|_| "not a save".to_string())?;
    check_record(&rec)?;
    if my_player_key(me, &rec).is_none() {
        return Err("another player's save".into());
    }
    Ok(rec)
}

/// Up to `n` friends to hold copies: the picked ones first, then the ones seen most lately.
pub fn pick_holders(friends: &[(String, String, u64)], picked: &[String], n: usize) -> Vec<(String, String)> {
    let mut v: Vec<&(String, String, u64)> = friends.iter().filter(|f| hex64(&f.0)).collect();
    v.sort_by_key(|f| (!picked.contains(&f.0), std::cmp::Reverse(f.2)));
    v.into_iter().take(n.min(HOLDERS_MAX)).map(|f| (f.0.clone(), f.1.clone())).collect()
}

/// Send `data` (base32) in parts: `{op, char, seq, part, of, data}`.
pub fn send_parts(link: &dyn FriendLink, to: &str, op: &str, char: &str, seq: u64, data: &str) -> Result<(), String> {
    let chunks: Vec<&str> = data.as_bytes().chunks(PART).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    if chunks.is_empty() || chunks.len() > PARTS_MAX {
        return Err("too big to send".into());
    }
    for (i, c) in chunks.iter().enumerate() {
        link.send(to, json!({"op": op, "char": char, "seq": seq, "part": i, "of": chunks.len(), "data": c}))?;
    }
    Ok(())
}

/// The newest save of a character sealed to its player and sent to up to the setting's number of friends, and its
/// witness to every friend. How many friends took the copy.
pub fn share_newest(app: &Path, link: &dyn FriendLink, id: &str, t: u64) -> Result<usize, String> {
    let rec = newest(app, id).ok_or("no save yet")?;
    let me = identity::load_or_create(app)?;
    let Some(signer) = my_player_key(&me, &rec) else { return Ok(0) };
    let settings = load_index(app).settings;
    let friends = link.friends();
    let blob = seal_save(&rec, &me.public_key)?;
    let seq = rec["save"]["seq"].as_u64().unwrap_or(0);
    let mut held = Vec::new();
    for (k, n) in pick_holders(&friends, &settings.picked, settings.holders) {
        if send_parts(link, &k, "keep", id, seq, &blob).is_ok() {
            held.push(Holder { key: k, name: n, seq, at: t });
        }
    }
    let w = make_witness(&signer, &rec, t);
    for (k, _, _) in friends.iter().filter(|f| hex64(&f.0)).take(20) {
        let _ = link.send(k, json!({"op": "witness", "witness": w}));
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = load_index(app);
    if let Some(e) = idx.chars.iter_mut().find(|e| e.id == id) {
        for h in &held {
            e.holders.retain(|x| x.key != h.key);
            e.holders.insert(0, h.clone());
        }
        e.holders.truncate(HOLDERS_MAX);
    }
    save_index(app, &idx)?;
    Ok(held.len())
}

/// "Get my characters back": ask every friend for the copies they hold of mine.
pub fn ask_back(link: &dyn FriendLink) -> usize {
    link.friends().iter().filter(|f| hex64(&f.0)).filter(|f| link.send(&f.0, json!({"op": "ask"})).is_ok()).count()
}

// ------------------------------------------------------------------------------------------------ what friends keep for me

fn held_dir(app: &Path) -> PathBuf {
    dir(app).join("held")
}

/// A friend's sealed copy kept here.
pub fn keep_held(app: &Path, owner: &str, char: &str, seq: u64, blob: &str, t: u64) -> Result<(), String> {
    if !hex64(owner) || !char_id_ok(char) || blob.len() > BLOB_MAX || identity::unb32(blob).is_none() {
        return Err("not a backup".into());
    }
    let pid = identity::peer_id(&identity::unhex32(owner).ok_or("key")?);
    let file = held_dir(app).join(&pid).join(format!("{char}.json"));
    if read_json(&file).is_some_and(|o| o["seq"].as_u64().unwrap_or(0) > seq) {
        return Ok(());
    }
    write_json(&file, &json!({"owner": owner, "char": char, "seq": seq, "blob": blob, "at": t}))?;
    trim_held(app, HELD_MAX);
    Ok(())
}

/// The copies kept here (file, size, when), and the oldest dropped while they are over `cap` in all.
pub fn trim_held(app: &Path, cap: u64) {
    let mut all: Vec<(PathBuf, u64, u64)> = Vec::new();
    for o in std::fs::read_dir(held_dir(app)).into_iter().flatten().flatten() {
        for f in std::fs::read_dir(o.path()).into_iter().flatten().flatten() {
            let size = f.metadata().map(|m| m.len()).unwrap_or(0);
            let at = read_json(&f.path()).and_then(|v| v["at"].as_u64()).unwrap_or(0);
            all.push((f.path(), size, at));
        }
    }
    let mut total: u64 = all.iter().map(|x| x.1).sum();
    all.sort_by_key(|x| x.2);
    for (p, size, _) in all {
        if total <= cap {
            break;
        }
        let _ = std::fs::remove_file(&p);
        total = total.saturating_sub(size);
    }
}

/// The copies held here for one owner.
pub fn held_for(app: &Path, owner: &str) -> Vec<Value> {
    let Some(pk) = identity::unhex32(owner) else { return Vec::new() };
    std::fs::read_dir(held_dir(app).join(identity::peer_id(&pk))).into_iter().flatten().flatten()
        .filter_map(|f| read_json(&f.path())).filter(|v| v["owner"] == owner).collect()
}

/// How much of friends' copies this PC keeps (bytes, owners).
pub fn held_size(app: &Path) -> (u64, usize) {
    let mut total = 0;
    let mut owners = 0;
    for o in std::fs::read_dir(held_dir(app)).into_iter().flatten().flatten() {
        owners += 1;
        for f in std::fs::read_dir(o.path()).into_iter().flatten().flatten() {
            total += f.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    (total, owners)
}

/// Witnesses kept here: newest per (player, char).
pub fn keep_witness(app: &Path, w: &Value) -> Result<bool, String> {
    check_witness(w)?;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let file = dir(app).join("witnesses.json");
    let mut all: BTreeMap<String, Value> = read_json(&file).and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let key = format!("{}:{}", w["witness"]["player"].as_str().unwrap_or(""), w["witness"]["char"].as_str().unwrap_or(""));
    if all.get(&key).is_some_and(|o| o["witness"]["seq"].as_u64() >= w["witness"]["seq"].as_u64()) {
        return Ok(false);
    }
    all.insert(key, w.clone());
    while all.len() > WITNESS_MAX {
        let oldest = all.iter().min_by_key(|(_, v)| v["witness"]["made"].as_u64().unwrap_or(0)).map(|(k, _)| k.clone());
        match oldest {
            Some(k) => all.remove(&k),
            None => break,
        };
    }
    write_json(&file, &serde_json::to_value(&all).unwrap_or_default())?;
    Ok(true)
}

pub fn witness_of(app: &Path, player: &str, char: &str) -> Option<Value> {
    read_json(&dir(app).join("witnesses.json")).and_then(|v| v.get(format!("{player}:{char}")).cloned())
}

/// One part in; the whole base32 text once every part is here.
pub fn take_part(app: &Path, from: &str, body: &Value, t: u64) -> Result<Option<String>, String> {
    let op = body["op"].as_str().unwrap_or("");
    let char = body["char"].as_str().unwrap_or("");
    let (seq, part, of) = (body["seq"].as_u64().unwrap_or(0), body["part"].as_u64().unwrap_or(u64::MAX), body["of"].as_u64().unwrap_or(0));
    let data = body["data"].as_str().unwrap_or("");
    if !char_id_ok(char) || seq == 0 || of == 0 || of as usize > PARTS_MAX || part >= of || data.is_empty() || data.len() > PART
        || !data.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b)) {
        return Err("part".into());
    }
    let inc = dir(app).join("incoming");
    // old and too many unfinished ones go
    let mut files: Vec<(PathBuf, u64)> = std::fs::read_dir(&inc).into_iter().flatten().flatten()
        .map(|f| (f.path(), read_json(&f.path()).and_then(|v| v["at"].as_u64()).unwrap_or(0))).collect();
    files.retain(|(p, at)| if t.saturating_sub(*at) > INCOMING_TTL { std::fs::remove_file(p).is_err() } else { true });
    let file = inc.join(format!("{}.json", &sha_hex(format!("{from}\n{op}\n{char}\n{seq}").as_bytes())[..24]));
    if files.len() >= INCOMING_MAX && !file.exists() {
        files.sort_by_key(|x| x.1);
        let _ = std::fs::remove_file(&files[0].0);
    }
    let mut cur = read_json(&file).filter(|v| v["of"].as_u64() == Some(of)).unwrap_or_else(|| json!({"of": of, "parts": vec![Value::Null; of as usize]}));
    cur["parts"][part as usize] = json!(data);
    cur["at"] = json!(t);
    let parts = cur["parts"].as_array().cloned().unwrap_or_default();
    if parts.iter().all(Value::is_string) {
        let _ = std::fs::remove_file(&file);
        return Ok(Some(parts.iter().filter_map(Value::as_str).collect()));
    }
    write_json(&file, &cur)?;
    Ok(None)
}

/// A `chars` message from a friend (checked by friendnet: signed by `from`, fresh, a friend's).
pub fn on_message(app: &Path, me: &PeerIdentity, from: &str, body: &Value, link: &dyn FriendLink, t: u64) -> Result<&'static str, String> {
    match body["op"].as_str().unwrap_or("") {
        "keep" => {
            if !load_index(app).settings.keep_for_friends {
                return Ok("not keeping friends' copies");
            }
            if let Some(blob) = take_part(app, from, body, t)? {
                keep_held(app, from, body["char"].as_str().unwrap_or(""), body["seq"].as_u64().unwrap_or(0), &blob, t)?;
                return Ok("kept a friend's copy");
            }
            Ok("part")
        }
        "ask" => {
            let mut n = 0;
            for h in held_for(app, from) {
                if send_parts(link, from, "back", h["char"].as_str().unwrap_or(""), h["seq"].as_u64().unwrap_or(0), h["blob"].as_str().unwrap_or("")).is_ok() {
                    n += 1;
                }
            }
            Ok(if n > 0 { "handed copies back" } else { "no copies held" })
        }
        "back" => {
            let Some(blob) = take_part(app, from, body, t)? else { return Ok("part") };
            let rec = open_save(me, &blob)?;
            restore(app, &rec)?;
            Ok("got a copy back")
        }
        "witness" => {
            let w = &body["witness"];
            // signed by the player's key for that world (a world key can't be tied to `from`, the friend's Passport,
            // by design): the signature is checked in keep_witness; only friends' messages reach here
            if w["witness"]["player"].as_str().is_none() {
                return Err("a witness with no player".into());
            }
            keep_witness(app, w)?;
            Ok("kept a witness")
        }
        "cosign" => {
            let Some(text) = take_part(app, from, body, t)? else { return Ok("part") };
            let rec: Value = serde_json::from_slice(&inflate(&identity::unb32(&text).ok_or("not a save")?, SAVE_MAX + 4096)?).map_err(|_| "not a save".to_string())?;
            let (seq, char, sig) = countersign(app, me, &rec, from)?;
            link.send(from, json!({"op": "cosigned", "char": char, "seq": seq, "hash": hash_doc(&rec["save"]), "sig": sig}))?;
            let ids = vec![char];
            for id in ids {
                let _ = share_newest(app, link, &id, t);
            }
            Ok("signed my save")
        }
        "cosigned" => {
            cosigned(app, from, body)?;
            Ok("save signed by its player")
        }
        _ => Ok("ignored"),
    }
}

/// The friend's side of a save of my character on a host's world: the host's signature checked, sent by that
/// world's host (`from` is the host's peer key, which signs the friends' messages; the save names the world key),
/// my key as the player, after my newest. Signed, kept. (seq, char id, my signature)
pub fn countersign(app: &Path, me: &PeerIdentity, rec: &Value, _from: &str) -> Result<(u64, String, String), String> {
    check_half(rec)?;
    let Some(signer) = my_player_key(me, rec) else {
        return Err("a save of someone else's character".into());
    };
    let mut full = rec.clone();
    let sig = signer.sign_doc(CTX_SAVE, &rec["save"]);
    full["player_sig"] = json!(sig);
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // a host may lose saves (a world restored from a back-up): a gap after my newest is taken; an older one isn't
    put_save(app, &full, true)?;
    let mut idx = load_index(app);
    let s = &full["save"];
    let id = s["char"].as_str().unwrap_or("").to_string();
    let i = match idx.chars.iter().position(|e| e.id == id) {
        Some(i) => i,
        None => {
            idx.chars.push(Entry { id: id.clone(), mine: true, ..Default::default() });
            idx.chars.len() - 1
        }
    };
    let e = &mut idx.chars[i];
    e.name = s["who"]["name"].as_str().unwrap_or("").into();
    e.realm = s["world"].as_str().unwrap_or("").into();
    e.seq = s["seq"].as_u64().unwrap_or(0);
    e.made = s["made"].as_u64().unwrap_or(0);
    let seq = e.seq;
    save_index(app, &idx)?;
    Ok((seq, id, sig))
}

/// The host's side: the player's countersignature on a pending half save.
pub fn cosigned(app: &Path, from: &str, body: &Value) -> Result<(), String> {
    let char = body["char"].as_str().unwrap_or("");
    if !char_id_ok(char) {
        return Err("char".into());
    }
    let file = char_dir(app, char).join("pending.json");
    let mut rec = read_json(&file).ok_or("no save waiting")?;
    // the player may sign with a world key that isn't `from` (their Passport): the signature itself is what counts
    // (check_record below, against the save's player key)
    let _ = from;
    if rec["save"]["seq"].as_u64() != body["seq"].as_u64() || body["hash"].as_str() != Some(hash_doc(&rec["save"]).as_str()) {
        return Err("not the save waiting".into());
    }
    rec["player_sig"] = body["sig"].clone();
    check_record(&rec)?;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    put_save(app, &rec, false)?;
    let _ = std::fs::remove_file(&file);
    let mut idx = load_index(app);
    if let Some(e) = idx.chars.iter_mut().find(|e| e.id == char) {
        e.seq = rec["save"]["seq"].as_u64().unwrap_or(0);
        e.made = rec["save"]["made"].as_u64().unwrap_or(0);
    }
    save_index(app, &idx)
}

/// A copy handed back by a friend: kept when it is newer than what this PC has (a new PC has nothing).
pub fn restore(app: &Path, rec: &Value) -> Result<bool, String> {
    check_record(rec)?;
    let s = &rec["save"];
    let id = s["char"].as_str().unwrap_or("").to_string();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let new = match put_save(app, rec, true) {
        Ok(n) => n,
        Err(e) if e.starts_with("an old save") => false,
        Err(e) => return Err(e),
    };
    let mut idx = load_index(app);
    if !idx.chars.iter().any(|e| e.id == id) {
        idx.chars.push(Entry { id: id.clone(), name: s["who"]["name"].as_str().unwrap_or("").into(), realm: s["world"].as_str().unwrap_or("").into(),
            mine: true, seq: s["seq"].as_u64().unwrap_or(0), made: s["made"].as_u64().unwrap_or(0), ..Default::default() });
    } else if new {
        if let Some(e) = idx.chars.iter_mut().find(|e| e.id == id) {
            e.seq = s["seq"].as_u64().unwrap_or(0);
            e.made = s["made"].as_u64().unwrap_or(0);
        }
    }
    save_index(app, &idx)?;
    Ok(new)
}

// ------------------------------------------------------------------------------------------------ re-home: customs

/// What a target world knows, read from its own database.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Target {
    pub world: String,
    pub title: String,
    pub ns: String,
    pub cap: u64,
    /// The original world: the first era's races and classes only.
    pub first_era: bool,
    /// entry: (required level, stack size, max durability, name)
    pub items: HashMap<u64, (u64, u64, u64, String)>,
    pub quests: HashSet<u64>,
    /// Where a new character of this race and class starts: map, zone, x, y, z, orientation (None: not offered).
    pub start: Option<(u64, u64, String, String, String, String)>,
    /// Names starting like the character's that are taken there.
    pub taken: Vec<String>,
    /// The player's characters on the login it goes to.
    pub count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Carried {
    pub entry: u64,
    pub n: u64,
    pub ench: Vec<(u64, u64)>,
    pub suffix: i64,
    pub durability: u64,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Vaulted {
    pub id: String,
    pub n: u64,
    pub why: String,
}

/// What comes, what goes to the vault and what stays behind.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Plan {
    pub ok: bool,
    pub why: String,
    pub char: String,
    pub seq: u64,
    pub name: String,
    /// The name is taken there: a free one is used and the game asks for a new name at first login.
    pub rename: bool,
    pub from_level: u64,
    pub level: u64,
    pub race: u64,
    pub class: u64,
    pub female: bool,
    pub looks: Vec<u64>,
    pub items: Vec<Carried>,
    pub vault: Vec<Vaulted>,
    pub done: Vec<u64>,
    pub rep: Vec<(u64, i64, u64)>,
    pub skills: Vec<(u64, u64, u64)>,
    pub spells: Vec<u64>,
    pub achievements: Vec<(u64, u64)>,
    pub stays: Vec<String>,
    pub comes: Vec<String>,
    pub at_login: u64,
}

/// Professions and riding: their skill ids (kept when the level is lowered; the rest the world sets again).
const PROFESSIONS: [u64; 15] = [129, 164, 165, 171, 182, 185, 186, 197, 202, 333, 356, 393, 755, 762, 773];
const AT_RENAME: u64 = 0x01;
const AT_RESET_SPELLS: u64 = 0x02;
const AT_RESET_TALENTS: u64 = 0x04;

fn refuse(mut p: Plan, why: impl Into<String>) -> Plan {
    p.ok = false;
    p.why = why.into();
    p
}

/// A free name like `want` (letters only, at most 12): `want` itself when it isn't taken.
pub fn free_name(want: &str, taken: &[String]) -> (String, bool) {
    let has = |n: &str| taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if !has(want) {
        return (want.to_string(), false);
    }
    let base: String = want.chars().take(10).collect();
    for a in b'a'..=b'z' {
        for b in b'a'..=b'z' {
            let n = format!("{base}{}{}", a as char, b as char);
            if !has(&n) {
                return (n, true);
            }
        }
    }
    (want.to_string(), true)
}

/// The customs of a world: a verified save's character as that world takes it.
pub fn customs(rec: &Value, t: &Target) -> Plan {
    let s = &rec["save"];
    let w = &s["who"];
    let ns = s["ns"].as_str().unwrap_or("");
    let mut p = Plan { char: s["char"].as_str().unwrap_or("").into(), seq: s["seq"].as_u64().unwrap_or(0), name: w["name"].as_str().unwrap_or("").into(), ..Default::default() };
    if let Err(e) = check_record(rec) {
        return refuse(p, format!("This save doesn't check out ({e})."));
    }
    let (Some(race), Some(class)) = (entry_of(w["race"].as_str().unwrap_or(""), ns, "race"), entry_of(w["class"].as_str().unwrap_or(""), ns, "class")) else {
        return refuse(p, "This save's race or class isn't one this world knows.");
    };
    p.race = race;
    p.class = class;
    p.female = w["gender"] == "female";
    p.looks = w["looks"].as_array().map(|a| a.iter().take(5).map(|x| x.as_u64().unwrap_or(0).min(255)).collect()).unwrap_or_default();
    p.looks.resize(5, 0);
    if ns != t.ns {
        return refuse(p, format!("{} is another kind of world: it can't make this character.", t.title));
    }
    if !crate::hybrid::char_name_ok(&p.name) {
        return refuse(p, "This save's name doesn't look right.");
    }
    let lr = crate::host::localworld::RACES.iter().find(|r| r.0 as u64 == race);
    let lc = crate::host::localworld::CLASSES.iter().find(|c| c.0 as u64 == class);
    let (Some(lr), Some(lc)) = (lr, lc) else { return refuse(p, "This save's race or class isn't one this world knows.") };
    if t.first_era && !(lr.3 && lc.3) {
        return refuse(p, format!("{} has no {} {}s: they came later.", t.title, lr.2, lc.2));
    }
    if t.start.is_none() {
        return refuse(p, format!("{} has no {} {}s.", t.title, lr.2, lc.2));
    }
    if t.count >= 10 {
        return refuse(p, format!("Your login in {} has 10 characters already: delete one there first.", t.title));
    }
    let (n, rename) = free_name(&p.name, &t.taken);
    p.rename = rename;
    let original = std::mem::replace(&mut p.name, n);
    p.from_level = w["level"].as_u64().unwrap_or(1).max(1);
    p.level = p.from_level.min(t.cap.max(1));
    let capped = p.level < p.from_level;
    p.at_login = AT_RESET_TALENTS | if capped { AT_RESET_SPELLS } else { 0 } | if rename { AT_RENAME } else { 0 };
    // items
    for it in w["items"].as_array().into_iter().flatten() {
        let id = it["id"].as_str().unwrap_or("").to_string();
        let n = it["n"].as_u64().unwrap_or(1).clamp(1, 100_000);
        let Some(e) = entry_of(&id, ns, "item") else {
            p.vault.push(Vaulted { id, n, why: "not an item this world knows".into() });
            continue;
        };
        let Some((req, stack, dur, label)) = t.items.get(&e) else {
            p.vault.push(Vaulted { id, n, why: format!("{} doesn't have it", t.title) });
            continue;
        };
        if *req > p.level {
            p.vault.push(Vaulted { id, n, why: format!("needs level {req}") });
            continue;
        }
        let ench: Vec<(u64, u64)> = it["ench"].as_array().into_iter().flatten()
            .filter_map(|x| Some((x[0].as_u64()?.min(11), entry_of(x[1].as_str()?, ns, "enchant")?))).filter(|(s, _)| *s != 1).collect();
        let stack = (*stack).max(1);
        let mut left = n;
        while left > 0 {
            let take = left.min(stack);
            if p.items.len() >= ITEMS_MAX {
                p.vault.push(Vaulted { id: id.clone(), n: left, why: "more than one move carries".into() });
                break;
            }
            p.items.push(Carried { entry: e, n: take, ench: ench.clone(), suffix: it["suffix"].as_i64().unwrap_or(0), durability: *dur, label: label.clone() });
            left -= take;
        }
    }
    p.done = w["quests"]["done"].as_array().into_iter().flatten().filter_map(|q| entry_of(q.as_str()?, ns, "quest")).filter(|q| t.quests.contains(q)).collect();
    p.rep = w["rep"].as_array().into_iter().flatten().filter_map(|r| Some((entry_of(r["id"].as_str()?, ns, "faction")?, r["standing"].as_i64()?.clamp(-42000, 42999), r["flags"].as_u64().unwrap_or(0) & 0xff))).collect();
    p.skills = w["skills"].as_array().into_iter().flatten()
        .filter_map(|r| Some((entry_of(r["id"].as_str()?, ns, "skill")?, r["value"].as_u64()?.min(450), r["max"].as_u64()?.min(450))))
        .filter(|(s, _, _)| !capped || PROFESSIONS.contains(s)).map(|(s, v, m)| (s, v.min(m), m)).collect();
    if !capped {
        p.spells = w["spells"].as_array().into_iter().flatten().filter_map(|x| entry_of(x.as_str()?, ns, "spell")).collect();
    }
    p.achievements = w["achievements"].as_array().into_iter().flatten().filter_map(|r| Some((entry_of(r["id"].as_str()?, ns, "achievement")?, r["date"].as_u64().unwrap_or(0)))).collect();
    // what stays behind, in plain words
    let money = w["money"].as_u64().unwrap_or(0);
    if money > 0 {
        p.stays.push(format!("Gold ({} gold): gold never travels.", money / 10_000));
    }
    let active = w["quests"]["active"].as_array().map(|a| a.len()).unwrap_or(0);
    if active > 0 {
        p.stays.push(format!("{active} quests in progress: pick them up again there."));
    }
    let raids = w["raids"].as_array().map(|a| a.len()).unwrap_or(0);
    if raids > 0 {
        p.stays.push(format!("{raids} raid lockouts: they belong to the old world."));
    }
    let lost_quests = w["quests"]["done"].as_array().map(|a| a.len()).unwrap_or(0).saturating_sub(p.done.len());
    if lost_quests > 0 {
        p.stays.push(format!("{lost_quests} finished quests this world doesn't have."));
    }
    if capped {
        p.stays.push(format!("Levels above {}: this world's highest level. Spells are learned again for level {}, and profession recipes stay behind.", p.level, p.level));
    }
    p.stays.push("Talents: you pick them again.".into());
    // what comes
    p.comes.push(format!("{} at level {}", p.name, p.level));
    if rename {
        p.comes.push(format!("{original} is taken there: the game asks you for a new name when you first log in."));
    }
    if !p.items.is_empty() {
        p.comes.push(format!("{} items, waiting in your mailbox", p.items.len()));
    }
    if !p.done.is_empty() {
        p.comes.push(format!("{} finished quests", p.done.len()));
    }
    if !p.rep.is_empty() {
        p.comes.push(format!("Standing with {} factions", p.rep.len()));
    }
    if !p.skills.is_empty() {
        p.comes.push(format!("{} skills", p.skills.len()));
    }
    if !p.spells.is_empty() {
        p.comes.push(format!("{} spells", p.spells.len()));
    }
    if !p.achievements.is_empty() {
        p.comes.push(format!("{} achievements", p.achievements.len()));
    }
    p.ok = true;
    p
}

fn num_ok(s: &str) -> Option<String> {
    s.trim().parse::<f64>().ok().filter(|x| x.is_finite()).map(|x| format!("{x}"))
}

fn values(rows: impl Iterator<Item = String>) -> Option<String> {
    let v: Vec<String> = rows.collect();
    (!v.is_empty()).then(|| v.join(", "))
}

/// The SQL that makes the planned character on the login `account` of the target world, in one transaction (the
/// mysql client stops at the first error, and an unfinished transaction is rolled back: never a half character).
/// Items come by in-game mail, 12 to a letter. Gold is 0.
pub fn import_sql(p: &Plan, account: &str, start: &(u64, u64, String, String, String, String), t: u64) -> Result<String, String> {
    if !p.ok || !crate::hybrid::char_name_ok(&p.name) || !(3..=16).contains(&account.len()) || !account.bytes().all(|b| b.is_ascii_alphanumeric()) || p.looks.len() != 5 {
        return Err("That character can't be made.".into());
    }
    let (map, zone) = (start.0, start.1);
    let (Some(x), Some(y), Some(z), Some(o)) = (num_ok(&start.2), num_ok(&start.3), num_ok(&start.4), num_ok(&start.5)) else {
        return Err("That world's start doesn't look right.".into());
    };
    let acc = account.to_ascii_uppercase();
    let mut s = String::new();
    s.push_str("START TRANSACTION;\n");
    s.push_str(&format!("SET @acc := (SELECT id FROM acore_auth.account WHERE username = '{acc}');\n"));
    s.push_str("SET @g := (SELECT COALESCE(MAX(guid), 0) + 1 FROM characters);\n");
    s.push_str("SET @i := (SELECT COALESCE(MAX(guid), 0) FROM item_instance);\n");
    s.push_str("SET @m := (SELECT COALESCE(MAX(id), 0) FROM mail);\n");
    let l = &p.looks;
    s.push_str(&format!(
        "INSERT INTO characters (guid, account, name, race, class, gender, level, xp, money, skin, face, hairStyle, hairColor, facialStyle, map, zone, position_x, position_y, position_z, orientation, taximask, cinematic, totaltime, leveltime, at_login, exploredZones, equipmentCache, knownTitles, online) \
         VALUES (@g, @acc, '{}', {}, {}, {}, {}, 0, 0, {}, {}, {}, {}, {}, {map}, {zone}, {x}, {y}, {z}, {o}, '', 1, 0, 0, {}, '', '', '', 0);\n",
        p.name, p.race, p.class, u8::from(p.female), p.level, l[0], l[1], l[2], l[3], l[4], p.at_login
    ));
    for (i, it) in p.items.iter().enumerate() {
        let mut e = [0u64; 36];
        for (slot, id) in &it.ench {
            e[(*slot as usize).min(11) * 3] = *id;
        }
        let ench: String = e.iter().map(|x| format!("{x} ")).collect();
        s.push_str(&format!(
            "INSERT INTO item_instance (guid, itemEntry, owner_guid, creatorGuid, giftCreatorGuid, count, duration, charges, flags, enchantments, randomPropertyId, durability, playedTime, text) \
             VALUES (@i + {}, {}, @g, 0, 0, {}, 0, '0 0 0 0 0 ', 0, '{ench}', {}, {}, 0, '');\n",
            i + 1, it.entry, it.n, it.suffix, it.durability
        ));
    }
    let expire = t + 10 * 365 * 86400;
    for (m, chunk) in p.items.chunks(MAIL_ITEMS).enumerate() {
        s.push_str(&format!(
            "INSERT INTO mail (id, messageType, stationery, mailTemplateId, sender, receiver, subject, body, has_items, expire_time, deliver_time, money, cod, checked) \
             VALUES (@m + {}, 0, 41, 0, @g, @g, 'Your belongings', 'Brought with you from your other world.', 1, {expire}, {t}, 0, 0, 0);\n",
            m + 1
        ));
        let first = m * MAIL_ITEMS;
        s.push_str(&format!("INSERT INTO mail_items (mail_id, item_guid, receiver) VALUES {};\n",
            (0..chunk.len()).map(|k| format!("(@m + {}, @i + {}, @g)", m + 1, first + k + 1)).collect::<Vec<_>>().join(", ")));
    }
    if let Some(v) = values(p.done.iter().map(|q| format!("(@g, {q}, 1)"))) {
        s.push_str(&format!("INSERT INTO character_queststatus_rewarded (guid, quest, active) VALUES {v};\n"));
    }
    if let Some(v) = values(p.rep.iter().map(|(f, st, fl)| format!("(@g, {f}, {st}, {fl})"))) {
        s.push_str(&format!("INSERT INTO character_reputation (guid, faction, standing, flags) VALUES {v};\n"));
    }
    if let Some(v) = values(p.skills.iter().map(|(sk, v, m)| format!("(@g, {sk}, {v}, {m})"))) {
        s.push_str(&format!("INSERT INTO character_skills (guid, skill, value, max) VALUES {v};\n"));
    }
    if let Some(v) = values(p.spells.iter().map(|sp| format!("(@g, {sp})"))) {
        s.push_str(&format!("INSERT INTO character_spell (guid, spell) VALUES {v};\n"));
    }
    if let Some(v) = values(p.achievements.iter().map(|(a, d)| format!("(@g, {a}, {d})"))) {
        s.push_str(&format!("INSERT INTO character_achievement (guid, achievement, date) VALUES {v};\n"));
    }
    s.push_str("REPLACE INTO acore_auth.realmcharacters (realmid, acctid, numchars) SELECT 1, @acc, (SELECT COUNT(*) FROM characters WHERE account = @acc);\n");
    s.push_str("COMMIT;\n");
    Ok(s)
}

// ------------------------------------------------------------------------------------------------ re-home: the worlds

/// The worlds a character can be brought into (Boss Rush's heroes are its own).
pub fn can_take(kind: Kind) -> bool {
    matches!(kind, Kind::Single | Kind::Vanilla)
}

pub fn level_cap(kind: Kind) -> u64 {
    match kind {
        Kind::Vanilla => 60,
        _ => 80,
    }
}

fn in_list(v: &[u64]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}

/// What a world knows of one save, from its database (`q(db, sql)`).
pub fn target_facts(kind: Kind, rec: &Value, account: &str, q: &dyn Fn(&str, &str) -> Result<String, String>) -> Result<Target, String> {
    let w = &rec["save"]["who"];
    let ns = rec["save"]["ns"].as_str().unwrap_or("");
    let mut t = Target { world: kind.word().into(), title: kind.title().into(), ns: NS.into(), cap: level_cap(kind), first_era: kind == Kind::Vanilla, ..Default::default() };
    let mut items: Vec<u64> = w["items"].as_array().into_iter().flatten().filter_map(|i| entry_of(i["id"].as_str()?, ns, "item")).collect();
    items.sort_unstable();
    items.dedup();
    if !items.is_empty() {
        for line in q("acore_world", &format!("SELECT entry, RequiredLevel, stackable, MaxDurability, name FROM item_template WHERE entry IN ({})", in_list(&items)))?.lines() {
            let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            let n = |i: usize| f[i].trim().parse::<i64>().unwrap_or(0).max(0) as u64;
            if let Ok(e) = f[0].trim().parse::<u64>() {
                t.items.insert(e, (n(1), n(2), n(3), f[4].chars().take(60).collect()));
            }
        }
    }
    let quests: Vec<u64> = w["quests"]["done"].as_array().into_iter().flatten().filter_map(|x| entry_of(x.as_str()?, ns, "quest")).collect();
    for chunk in quests.chunks(1000) {
        for line in q("acore_world", &format!("SELECT ID FROM quest_template WHERE ID IN ({})", in_list(chunk)))?.lines() {
            if let Ok(id) = line.trim().parse() {
                t.quests.insert(id);
            }
        }
    }
    if let (Some(r), Some(c)) = (entry_of(w["race"].as_str().unwrap_or(""), ns, "race"), entry_of(w["class"].as_str().unwrap_or(""), ns, "class")) {
        let row = q("acore_world", &format!("SELECT map, zone, position_x, position_y, position_z, orientation FROM playercreateinfo WHERE race = {r} AND class = {c}"))?;
        if let Some(line) = row.lines().next() {
            let f: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
            if f.len() >= 6 {
                if let (Ok(m), Ok(z)) = (f[0].trim().parse(), f[1].trim().parse()) {
                    t.start = Some((m, z, f[2].trim().into(), f[3].trim().into(), f[4].trim().into(), f[5].trim().into()));
                }
            }
        }
    }
    let nm = w["name"].as_str().unwrap_or("");
    if crate::hybrid::char_name_ok(nm) {
        let base: String = nm.chars().take(10).collect();
        t.taken = q("acore_characters", &format!("SELECT name FROM characters WHERE name LIKE '{base}%'"))?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    }
    if (3..=16).contains(&account.len()) && account.bytes().all(|b| b.is_ascii_alphanumeric()) {
        t.count = q("acore_characters", &format!("SELECT COUNT(*) FROM characters c JOIN acore_auth.account a ON a.id = c.account WHERE a.username = '{}'", account.to_ascii_uppercase()))?
            .trim().parse().unwrap_or(0);
    }
    Ok(t)
}

/// The login a character is brought to in a world: the one Play uses first.
fn target_account(p: &eng::Paths) -> String {
    eng::own_logins(&eng::load_state(p)).into_iter().next().unwrap_or_default()
}

/// The newest save of one of my characters, checked.
fn my_newest(app: &Path, id: &str) -> Result<Value, String> {
    let rec = newest(app, id).ok_or("There is no save of that character on this PC yet.")?;
    let me = identity::load_or_create(app)?;
    if my_player_key(&me, &rec).is_none() {
        return Err("That character's saves aren't yours.".into());
    }
    Ok(rec)
}

fn lives_there(app: &Path, id: &str, kind: Kind) -> bool {
    load_index(app).chars.iter().any(|e| e.id == id && e.mine && e.world == kind.word())
}

/// The preview of "Bring to another world": what comes, what goes to the vault, what stays.
pub fn preview(app: &Path, id: &str, to: Kind) -> Result<Plan, String> {
    if !can_take(to) {
        return Err(format!("Characters can't be brought into {}.", to.title()));
    }
    let rec = my_newest(app, id)?;
    if lives_there(app, id, to) {
        return Err(format!("That character lives in {} already.", to.title()));
    }
    let p = eng::Paths::for_kind(app, to);
    if !eng::status(&p).set_up {
        return Err(format!("Set {} up first.", to.title()));
    }
    if let Some(o) = eng::other_running(&p) {
        return Err(format!("{} is running on this PC: stop it first, then look again.", o.title()));
    }
    let account = target_account(&p);
    let t = eng::with_db(&p, || target_facts(to, &rec, &account, &|db, sql| eng::query(&p, db, sql)))?;
    Ok(customs(&rec, &t))
}

/// "Bring": the character made in the other world from its newest save, after a back-up of that world. The worlds
/// must be stopped (a running world hands out the same row numbers).
pub fn bring(app: &Path, id: &str, to: Kind) -> Result<String, String> {
    if !can_take(to) {
        return Err(format!("Characters can't be brought into {}.", to.title()));
    }
    let rec = my_newest(app, id)?;
    if lives_there(app, id, to) {
        return Err(format!("That character lives in {} already.", to.title()));
    }
    let p = eng::Paths::for_kind(app, to);
    if !eng::status(&p).set_up {
        return Err(format!("Set {} up first.", to.title()));
    }
    if eng::port_answers(eng::WORLD_PORT) || eng::other_running(&p).is_some() {
        return Err("Stop the world that is running first: characters are brought in while worlds are stopped.".into());
    }
    let account = target_account(&p);
    if account.is_empty() {
        return Err(format!("Play {} once first.", to.title()));
    }
    eng::backup_why(&p, "before bringing a character").map_err(|e| format!("Nothing was brought: the back-up before it didn't work ({e})."))?;
    let t0 = now();
    let (plan, guid) = eng::with_db(&p, || {
        let t = target_facts(to, &rec, &account, &|db, sql| eng::query(&p, db, sql))?;
        let plan = customs(&rec, &t);
        if !plan.ok {
            return Err(plan.why.clone());
        }
        let start = t.start.clone().ok_or("no start")?;
        eng::sql(&p, "acore_characters", &import_sql(&plan, &account, &start, t0)?).map_err(|e| format!("Nothing was brought ({e})."))?;
        let guid: u64 = eng::query(&p, "acore_characters", &format!("SELECT c.guid FROM characters c JOIN acore_auth.account a ON a.id = c.account WHERE c.name = '{}' AND a.username = '{}'", plan.name, account.to_ascii_uppercase()))?
            .lines().next().and_then(|l| l.trim().parse().ok()).ok_or("The character was made but couldn't be found again.")?;
        Ok((plan, guid))
    })?;
    eng::setup_log(&p, &format!("a character was brought in from a signed save ({} items by mail, {} to the vault; a back-up was taken first)", plan.items.len(), plan.vault.len()));
    let realm = LocalKeys { app: app.to_path_buf() }.world(to).map(|k| identity::peer_id(&k.public_key)).unwrap_or_default();
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = load_index(app);
    if let Some(e) = idx.chars.iter_mut().find(|e| e.id == id && e.mine) {
        let old = format!("{}:{}", e.realm, e.guid);
        if !e.homes.contains(&old) && !e.realm.is_empty() {
            e.homes.push(old);
        }
        e.world = to.word().into();
        e.realm = realm;
        e.guid = guid;
        e.name = plan.name.clone();
    }
    for v in &plan.vault {
        idx.vault.push(VaultItem { char: id.into(), id: v.id.clone(), n: v.n, why: v.why.clone(), at: t0 });
    }
    save_index(app, &idx)?;
    Ok(format!(
        "{} is in {} now.{}{} Press Play there.",
        plan.name,
        to.title(),
        if plan.items.is_empty() { String::new() } else { " Your things wait in the mailbox.".into() },
        if plan.vault.is_empty() { String::new() } else { format!(" {} went to your vault.", plan.vault.len()) },
    ))
}

// ------------------------------------------------------------------------------------------------ the page's view

#[derive(Debug, Clone, Serialize)]
pub struct CharView {
    pub id: String,
    pub name: String,
    pub world: String,
    pub seq: u64,
    pub made: u64,
    pub holders: Vec<String>,
    pub vault: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub chars: Vec<CharView>,
    pub holders: usize,
    pub keep_for_friends: bool,
    pub held_bytes: u64,
    pub held_owners: usize,
}

pub fn view(app: &Path) -> View {
    let idx = load_index(app);
    let (held_bytes, held_owners) = held_size(app);
    View {
        chars: idx.chars.iter().filter(|e| e.mine).map(|e| CharView {
            id: e.id.clone(),
            name: e.name.clone(),
            world: e.world.clone(),
            seq: e.seq,
            made: e.made,
            holders: e.holders.iter().map(|h| if h.name.is_empty() { format!("Hero {}", &identity::peer_id(&identity::unhex32(&h.key).unwrap_or([0; 32]))[..4]) } else { h.name.clone() }).collect(),
            vault: idx.vault.iter().filter(|v| v.char == e.id).count(),
        }).collect(),
        holders: idx.settings.holders,
        keep_for_friends: idx.settings.keep_for_friends,
        held_bytes,
        held_owners,
    }
}

pub fn set_settings(app: &Path, holders: Option<usize>, keep: Option<bool>) -> Result<(), String> {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut idx = load_index(app);
    if let Some(h) = holders {
        idx.settings.holders = h.min(HOLDERS_MAX);
    }
    if let Some(k) = keep {
        idx.settings.keep_for_friends = k;
    }
    save_index(app, &idx)
}

// ------------------------------------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn key(n: u8) -> PeerIdentity {
        PeerIdentity::from_seed([n; 32]).unwrap()
    }

    struct TestKeys {
        player: u8,
        world: u8,
    }
    impl Keys for TestKeys {
        fn player(&self, _realm: &str) -> Result<PeerIdentity, String> {
            Ok(key(self.player))
        }
        fn world(&self, kind: Kind) -> Result<PeerIdentity, String> {
            Ok(key(self.world + kind as u8))
        }
    }

    /// The rows a world's database answers for one character (Tamsin, guid 7).
    fn fixture(sql: &str) -> Result<String, String> {
        if sql.starts_with("SELECT c.guid FROM") {
            return Ok("7\n".into());
        }
        if sql.contains("c.facialStyle") {
            return Ok(if sql.contains("c.guid = 7 ") { "Tamsin\t4\t11\t1\t70\t1200\t5000000\t36000\t1\t2\t3\t4\t5\tSOLO\n".into() } else { String::new() });
        }
        if sql.contains("UNION ALL") {
            return Ok([
                // worn sword with an enchantment (slot 0) and a temporary one (slot 1, left out), bag potions, a bank item
                "i\t19019\t1\t0\t0\t15\t1900 0 0 2564 3600 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ",
                "i\t13446\t45\t0\t0\t23\t0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ",
                "i\t90001\t1\t-5\t0\t40\t",
                "i\t34000\t1\t0\t0\t24\t",
                "q\t100\t0\t0\t0\t0\t", "q\t200\t0\t0\t0\t0\t", "q\t999\t0\t0\t0\t0\t",
                "a\t300\t3\t0\t0\t0\t",
                "r\t72\t3000\t17\t0\t0\t",
                "s\t171\t300\t375\t0\t0\t", "s\t43\t350\t350\t0\t0\t",
                "p\t133\t0\t0\t0\t0\t", "p\t2054\t0\t0\t0\t0\t",
                "h\t6\t1700000000\t0\t0\t0\t",
                "d\t409\t0\t1800000000\t0\t0\t",
            ].join("\n") + "\n");
        }
        Err(format!("unexpected: {sql}"))
    }

    /// The target world's database (the original world: cap 60, knows some items and quests).
    fn target_db(db: &str, sql: &str) -> Result<String, String> {
        match db {
            "acore_world" if sql.contains("item_template") => Ok("19019\t60\t1\t125\tA sword\n13446\t45\t5\t0\tA potion\n34000\t70\t1\t0\tLater gear\n".into()),
            "acore_world" if sql.contains("quest_template") => Ok("100\n200\n".into()),
            "acore_world" if sql.contains("playercreateinfo") => Ok(if sql.contains("race = 4 AND class = 11") { "1\t141\t10311.3\t832.463\t1326.41\t5.69632\n".into() } else { String::new() }),
            "acore_characters" if sql.contains("LIKE") => Ok("Tamsin\n".into()),
            "acore_characters" if sql.contains("COUNT(*)") => Ok("2\n".into()),
            _ => Err(format!("unexpected: {db} {sql}")),
        }
    }

    fn one_save(app: &Path, keys: &TestKeys) -> Value {
        let ids = make_saves_with(app, Kind::Single, &fixture, &[7], keys, 1_000).unwrap();
        assert_eq!(ids.len(), 1);
        newest(app, &ids[0]).unwrap()
    }

    #[test]
    fn a_character_is_read_by_ids_and_numbers_only() {
        let st = read_char(&fixture, 7).unwrap();
        assert_eq!((st.name.as_str(), st.race, st.class, st.female, st.level, st.money, st.account.as_str()), ("Tamsin", 4, 11, true, 70, 5_000_000, "SOLO"));
        assert_eq!(st.items.len(), 4);
        let sword = st.items.iter().find(|i| i.entry == 19019).unwrap();
        assert_eq!((sword.place, sword.enchants.clone()), ("worn", vec![(0, 1900)]), "the temporary enchantment is left out");
        assert_eq!(st.items.iter().find(|i| i.entry == 90001).unwrap().place, "bank");
        assert_eq!(st.items.iter().find(|i| i.entry == 90001).unwrap().suffix, -5);
        assert_eq!((st.done.clone(), st.active.clone(), st.spells.clone()), (vec![100, 200, 999], vec![300], vec![133, 2054]));
        assert_eq!(st.raids, vec![(409, 0, 1_800_000_000)]);
        let c = content(&st);
        assert_eq!(c["race"], "stock-335a:race:4");
        assert_eq!(c["items"].as_array().unwrap().iter().find(|i| i["id"] == "stock-335a:item:19019").unwrap()["ench"], json!([[0, "stock-335a:enchant:1900"]]));
        assert_eq!(c["quests"]["done"][0], "stock-335a:quest:100");
        assert_eq!(c["raids"][0]["map"], "stock-335a:map:409");
        assert!(!identity::canonical(&c).contains('.'), "no floating-point numbers");
        assert!(read_char(&fixture, 8).is_err());
        assert_eq!(entry_of("stock-335a:item:19019", NS, "item"), Some(19019));
        assert_eq!(entry_of("other:item:19019", NS, "item"), None);
        assert_eq!(entry_of("stock-335a:quest:19019", NS, "item"), None);
        assert!(guids_sql(&["solo".into()]).unwrap().contains("IN ('SOLO')") && guids_sql(&["x'; DROP".into()]).is_err());
    }

    #[test]
    fn a_save_is_signed_by_both_keys_and_chained() {
        let d = tempfile::tempdir().unwrap();
        let keys = TestKeys { player: 1, world: 10 };
        let r1 = one_save(d.path(), &keys);
        check_record(&r1).unwrap();
        let s = &r1["save"];
        assert_eq!((s["seq"].as_u64(), s["prev"].as_str()), (Some(1), Some("")));
        assert_eq!(s["world"].as_str().unwrap(), identity::peer_id(&key(10).public_key));
        assert_eq!(s["player"].as_str().unwrap(), key(1).public_hex());
        assert!(char_id_ok(s["char"].as_str().unwrap()));
        // nothing changed: no new save
        assert!(make_saves_with(d.path(), Kind::Single, &fixture, &[7], &keys, 2_000).unwrap().is_empty());
        // a change makes the next one, pointing at the first
        let changed = |sql: &str| fixture(sql).map(|r| r.replace("\t70\t1200\t", "\t71\t0\t"));
        let id = s["char"].as_str().unwrap().to_string();
        assert_eq!(make_saves_with(d.path(), Kind::Single, &changed, &[7], &keys, 3_000).unwrap(), vec![id.clone()]);
        let all = saves(d.path(), &id);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1]["save"]["prev"].as_str().unwrap(), hash_doc(&all[0]["save"]));
        check_chain(&all).unwrap();
        // out of order, and a chain with a save in between left out
        assert!(check_chain(&[all[1].clone(), all[0].clone()]).unwrap_err().contains("out of order"));
        let mut skip = all[1].clone();
        skip["save"]["seq"] = json!(3);
        let skip = make_record(skip["save"].clone(), Some(&key(10)), Some(&key(1)));
        assert!(check_chain(&[all[0].clone(), skip]).is_err());
        let idx = load_index(d.path());
        assert_eq!((idx.chars[0].name.as_str(), idx.chars[0].world.as_str(), idx.chars[0].seq), ("Tamsin", "single", 2));
    }

    #[test]
    fn a_changed_or_half_forged_save_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let r = one_save(d.path(), &TestKeys { player: 1, world: 10 });
        let mut t = r.clone();
        t["save"]["who"]["level"] = json!(80);
        assert!(check_record(&t).unwrap_err().contains("signature"), "tampered");
        let mut gold = r.clone();
        gold["save"]["who"]["money"] = json!(999_999_999);
        assert!(check_record(&gold).is_err());
        // the host's signature is real; the player's is made by someone else's key
        let mut forged = r.clone();
        forged["player_sig"] = json!(key(9).sign_doc(CTX_SAVE, &r["save"]));
        assert!(check_record(&forged).unwrap_err().contains("player"));
        // only one signature
        let mut one = r.clone();
        one["player_sig"] = json!("");
        assert!(check_record(&one).is_err());
        check_half(&one).unwrap();
        // a world that isn't the host key's
        let mut w = r["save"].clone();
        w["world"] = json!(identity::peer_id(&key(9).public_key));
        assert!(check_record(&make_record(w, Some(&key(10)), Some(&key(1)))).unwrap_err().contains("world"));
        // a host signature by the player's key
        let mut h = r.clone();
        h["host_sig"] = json!(key(1).sign_doc(CTX_SAVE, &r["save"]));
        assert!(check_record(&h).unwrap_err().contains("host"));
        assert!(check_record(&json!({"kind": "char-save"})).is_err());
    }

    #[test]
    fn an_old_save_shown_again_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let keys = TestKeys { player: 1, world: 10 };
        let r1 = one_save(d.path(), &keys);
        let changed = |sql: &str| fixture(sql).map(|r| r.replace("\t70\t1200\t", "\t72\t0\t"));
        make_saves_with(d.path(), Kind::Single, &changed, &[7], &keys, 3_000).unwrap();
        let id = r1["save"]["char"].as_str().unwrap();
        let r2 = newest(d.path(), id).unwrap();
        let known = (2, hash_doc(&r2["save"]));
        assert!(check_next(Some((known.0, &known.1)), &r1, true).unwrap_err().contains("old save"));
        assert_eq!(check_next(Some((known.0, &known.1)), &r2, false), Ok(false), "the same save again: nothing new");
        assert!(put_save(d.path(), &r1, true).unwrap_err().contains("old save"));
        // a fork: another seq 2
        let mut f = r2["save"].clone();
        f["who"]["level"] = json!(73);
        let fork = make_record(f, Some(&key(10)), Some(&key(1)));
        assert!(check_next(Some((known.0, &known.1)), &fork, true).unwrap_err().contains("fork"));
        // seq 3 that doesn't point at the newest
        let mut g = r2["save"].clone();
        g["seq"] = json!(3);
        g["prev"] = json!(hash_doc(&r1["save"]));
        assert!(check_next(Some((known.0, &known.1)), &make_record(g, Some(&key(10)), Some(&key(1))), true).unwrap_err().contains("fork"));
        // witnesses: the newest proven, the old one stale
        let w = make_witness(&key(1), &r2, 3_000);
        check_witness(&w).unwrap();
        assert_eq!(judge(&w, &r2), Verdict::Newest);
        assert_eq!(judge(&w, &r1), Verdict::Stale);
        assert_eq!(judge(&w, &fork), Verdict::Fork);
        assert_eq!(judge(&make_witness(&key(1), &r1, 1), &r2), Verdict::Ahead);
        let mut bad = w.clone();
        bad["witness"]["seq"] = json!(9);
        assert!(check_witness(&bad).is_err());
        assert!(keep_witness(d.path(), &w).unwrap());
        assert!(!keep_witness(d.path(), &make_witness(&key(1), &r1, 1)).unwrap(), "an older witness doesn't replace a newer");
        assert_eq!(witness_of(d.path(), &key(1).public_hex(), id).unwrap()["witness"]["seq"], 2);
    }

    #[test]
    fn only_the_last_twenty_are_kept() {
        let d = tempfile::tempdir().unwrap();
        let keys = TestKeys { player: 1, world: 10 };
        for lvl in 1..=25 {
            let f = move |sql: &str| fixture(sql).map(|r| r.replace("\t70\t1200\t", &format!("\t{lvl}\t0\t")));
            make_saves_with(d.path(), Kind::Single, &f, &[7], &keys, 1_000 + lvl).unwrap();
        }
        let id = load_index(d.path()).chars[0].id.clone();
        let all = saves(d.path(), &id);
        assert_eq!(all.len(), KEEP);
        assert_eq!(all.last().unwrap()["save"]["seq"], 25);
        check_chain(&all).unwrap();
    }

    struct FakeLink {
        friends: Vec<(String, String, u64)>,
        sent: RefCell<Vec<(String, Value)>>,
    }
    impl FriendLink for FakeLink {
        fn friends(&self) -> Vec<(String, String, u64)> {
            self.friends.clone()
        }
        fn send(&self, to: &str, body: Value) -> Result<(), String> {
            self.sent.borrow_mut().push((to.into(), body));
            Ok(())
        }
    }

    fn with_key(app: &Path, n: u8) {
        std::fs::create_dir_all(app).unwrap();
        std::fs::write(identity::key_path(app), identity::hex(&[n; 32]) + "\n").unwrap();
    }

    #[test]
    fn friends_keep_a_sealed_copy_they_cant_read_and_hand_it_back() {
        let (a, b, c, a2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        with_key(a.path(), 1);
        with_key(a2.path(), 1);
        let keys = TestKeys { player: 1, world: 10 };
        let r = one_save(a.path(), &keys);
        let id = r["save"]["char"].as_str().unwrap().to_string();
        let fl = |n: u8, seen: u64| (key(n).public_hex(), format!("F{n}"), seen);
        let link = FakeLink { friends: vec![fl(2, 5), fl(3, 9), fl(4, 1), fl(5, 7)], sent: RefCell::new(Vec::new()) };
        assert_eq!(share_newest(a.path(), &link, &id, 100).unwrap(), 3, "three friends by default");
        let sent = link.sent.borrow().clone();
        let keeps: Vec<&(String, Value)> = sent.iter().filter(|(_, b)| b["op"] == "keep").collect();
        let to: HashSet<&str> = keeps.iter().map(|(t, _)| t.as_str()).collect();
        assert!(to.contains(key(3).public_hex().as_str()) && to.contains(key(5).public_hex().as_str()) && to.contains(key(2).public_hex().as_str()), "the ones seen most lately");
        assert_eq!(sent.iter().filter(|(_, b)| b["op"] == "witness").count(), 4, "a witness for every friend");
        assert_eq!(load_index(a.path()).chars[0].holders.len(), 3);
        // friend 2 (B) gets the parts and keeps the blob
        let none = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        for (t, body) in &sent {
            if *t == key(2).public_hex() {
                on_message(b.path(), &key(2), &key(1).public_hex(), body, &none, 101).unwrap();
            }
        }
        let held = held_for(b.path(), &key(1).public_hex());
        assert_eq!(held.len(), 1);
        let blob = held[0]["blob"].as_str().unwrap();
        assert!(!blob.contains("Tamsin"));
        assert!(open_save(&key(2), blob).is_err(), "the friend can't open it");
        assert!(witness_of(b.path(), &key(1).public_hex(), &id).is_some());
        // a friend who turned keeping off keeps nothing
        set_settings(c.path(), None, Some(false)).unwrap();
        for (t, body) in &sent {
            if *t == key(3).public_hex() && body["op"] == "keep" {
                assert_eq!(on_message(c.path(), &key(3), &key(1).public_hex(), body, &none, 101).unwrap(), "not keeping friends' copies");
            }
        }
        // someone else asking gets nothing
        let back = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        assert_eq!(on_message(b.path(), &key(2), &key(9).public_hex(), &json!({"op": "ask"}), &back, 200).unwrap(), "no copies held");
        // the owner on a new PC (the same key brought back) asks, and gets it
        assert_eq!(on_message(b.path(), &key(2), &key(1).public_hex(), &json!({"op": "ask"}), &back, 200).unwrap(), "handed copies back");
        let mut last = "";
        for (_, body) in back.sent.borrow().iter() {
            last = on_message(a2.path(), &key(1), &key(2).public_hex(), body, &none, 201).unwrap();
        }
        assert_eq!(last, "got a copy back");
        assert_eq!(newest(a2.path(), &id).unwrap(), r);
        assert_eq!(view(a2.path()).chars[0].name, "Tamsin");
        // a witness passed on by someone else is only kept when the player's own signature holds (a world key can't
        // be tied to the sender, so the signature is the check)
        let mut w = make_witness(&key(1), &r, 1);
        w["witness"]["seq"] = json!(99);
        assert!(on_message(b.path(), &key(2), &key(9).public_hex(), &json!({"op": "witness", "witness": w}), &none, 1).is_err());
    }

    #[test]
    fn the_player_signs_with_the_passports_world_key_and_the_phrase_alone_brings_it_back() {
        let (a, b, a2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        with_key(a.path(), 1);
        with_key(a2.path(), 1);
        let keys = LocalKeys { app: a.path().to_path_buf() };
        let ids = make_saves_with(a.path(), Kind::Single, &fixture, &[7], &keys, 1_000).unwrap();
        let id = ids[0].clone();
        let r = newest(a.path(), &id).unwrap();
        check_record(&r).unwrap();
        let world = r["save"]["world"].as_str().unwrap();
        let wk = crate::passport::world_identity(&key(1), world);
        assert_eq!(r["save"]["player"].as_str(), Some(wk.public_hex().as_str()), "the world key, not the Passport root");
        assert_ne!(wk.public_key, key(1).public_key);
        assert_eq!(my_player_key(&key(1), &r).map(|k| k.public_key), Some(wk.public_key));
        assert!(my_player_key(&key(2), &r).is_none(), "another Passport's");
        assert!(my_newest(a.path(), &id).is_ok());
        // a friend keeps the sealed copy and the world-key witness; a new PC with only the root gets it back
        let link = FakeLink { friends: vec![(key(2).public_hex(), "F2".into(), 5)], sent: RefCell::new(Vec::new()) };
        assert_eq!(share_newest(a.path(), &link, &id, 100).unwrap(), 1);
        let none = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        for (_, body) in link.sent.borrow().iter() {
            on_message(b.path(), &key(2), &key(1).public_hex(), body, &none, 101).unwrap();
        }
        assert!(witness_of(b.path(), &wk.public_hex(), &id).is_some(), "the witness is under the world key");
        let back = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        assert_eq!(on_message(b.path(), &key(2), &key(1).public_hex(), &json!({"op": "ask"}), &back, 200).unwrap(), "handed copies back");
        let mut last = "";
        for (_, body) in back.sent.borrow().iter() {
            last = on_message(a2.path(), &key(1), &key(2).public_hex(), body, &none, 201).unwrap();
        }
        assert_eq!(last, "got a copy back");
        assert_eq!(newest(a2.path(), &id).unwrap(), r);
        // a host's half save for my world key is countersigned with that world key
        let host = key(30);
        let realm = identity::peer_id(&host.public_key);
        let st = read_char(&fixture, 7).unwrap();
        let mine = crate::passport::world_identity(&key(1), &realm);
        let half = make_record(save_doc(&st, None, &char_id(&realm, 7), &realm, &mine.public_hex(), &host.public_hex(), 5), Some(&host), None);
        let (_, _, sig) = countersign(a2.path(), &key(1), &half, &host.public_hex()).unwrap();
        let mut full = half.clone();
        full["player_sig"] = json!(sig);
        check_record(&full).unwrap();
    }

    #[test]
    fn friends_copies_stay_under_the_cap_newest_first() {
        let d = tempfile::tempdir().unwrap();
        let blob = "a".repeat(4000);
        for n in 1..=6u8 {
            keep_held(d.path(), &key(n).public_hex(), &char_id("r", n as u64), 1, &blob, n as u64).unwrap();
        }
        trim_held(d.path(), 12_000);
        let (size, _) = held_size(d.path());
        assert!(size <= 12_000);
        assert!(held_for(d.path(), &key(6).public_hex()).len() == 1 && held_for(d.path(), &key(1).public_hex()).is_empty(), "the oldest went first");
        // an older seq doesn't replace a newer one
        keep_held(d.path(), &key(6).public_hex(), &char_id("r", 6), 5, "bbbbbbbb", 10).unwrap();
        keep_held(d.path(), &key(6).public_hex(), &char_id("r", 6), 4, "cccccccc", 11).unwrap();
        assert_eq!(held_for(d.path(), &key(6).public_hex())[0]["blob"], "bbbbbbbb");
        assert!(keep_held(d.path(), &key(6).public_hex(), "bad", 1, "aaaa", 1).is_err());
        assert!(keep_held(d.path(), &key(6).public_hex(), &char_id("r", 6), 9, "not base32!", 1).is_err());
        // parts: bad ones refused, out of order fine
        let p = |part: u64, data: &str| json!({"op": "keep", "char": char_id("r", 1), "seq": 1, "part": part, "of": 2, "data": data});
        assert!(take_part(d.path(), "x", &p(2, "ab"), 1).is_err());
        assert!(take_part(d.path(), "x", &p(0, "AB"), 1).is_err());
        assert_eq!(take_part(d.path(), "x", &p(1, "cd"), 1).unwrap(), None);
        assert_eq!(take_part(d.path(), "x", &p(0, "ab"), 1).unwrap(), Some("abcd".into()));
        assert_eq!(pick_holders(&[(key(1).public_hex(), "".into(), 1), (key(2).public_hex(), "".into(), 9)], &[key(1).public_hex()], 1)[0].0, key(1).public_hex(), "picked first");
    }

    #[test]
    fn a_friends_character_is_signed_by_the_host_then_the_friend() {
        let (host, friend) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        with_key(friend.path(), 3);
        let st = read_char(&fixture, 7).unwrap();
        let hk = TestKeys { player: 1, world: 10 };
        let half = half_save(host.path(), Kind::Single, &st, &key(3).public_hex(), &hk, 50).unwrap().unwrap();
        check_half(&half).unwrap();
        assert!(check_record(&half).is_err());
        assert!(load_index(host.path()).chars.iter().all(|e| !e.mine));
        // sent in parts to the friend, who signs it and answers
        let to_friend = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        let id = half["save"]["char"].as_str().unwrap().to_string();
        send_parts(&to_friend, &key(3).public_hex(), "cosign", &id, 1, &identity::b32(&deflate(identity::canonical(&half).as_bytes()))).unwrap();
        let answers = FakeLink { friends: vec![], sent: RefCell::new(Vec::new()) };
        for (_, b) in to_friend.sent.borrow().iter() {
            on_message(friend.path(), &key(3), &key(1).public_hex(), b, &answers, 51).unwrap();
        }
        let mine = newest(friend.path(), &id).unwrap();
        check_record(&mine).unwrap();
        assert!(view(friend.path()).chars.iter().any(|c| c.id == id && c.name == "Tamsin"));
        let reply = answers.sent.borrow().iter().find(|(_, b)| b["op"] == "cosigned").cloned().unwrap();
        on_message(host.path(), &key(1), &key(3).public_hex(), &reply.1, &to_friend, 52).unwrap();
        assert_eq!(newest(host.path(), &id).unwrap(), mine, "both sides keep the same twice-signed save");
        // someone else's countersignature, or a save naming another player, is refused
        let other = half_save(host.path(), Kind::Single, &CharState { level: 2, ..st.clone() }, &key(3).public_hex(), &hk, 60).unwrap().unwrap();
        assert!(countersign(friend.path(), &key(4), &other, "").is_err());
        let mut fake = reply.1.clone();
        fake["seq"] = json!(2);
        fake["hash"] = json!(hash_doc(&other["save"]));
        fake["sig"] = json!(key(4).sign_doc(CTX_SAVE, &other["save"]));
        assert!(cosigned(host.path(), &key(3).public_hex(), &fake).is_err());
    }

    fn target_for(rec: &Value) -> Target {
        target_facts(Kind::Vanilla, rec, "ORIGINAL", &target_db).unwrap()
    }

    #[test]
    fn customs_keep_the_worlds_rules() {
        let d = tempfile::tempdir().unwrap();
        let r = one_save(d.path(), &TestKeys { player: 1, world: 10 });
        let t = target_for(&r);
        assert_eq!((t.cap, t.count, t.quests.len()), (60, 2, 2));
        let p = customs(&r, &t);
        assert!(p.ok, "{}", p.why);
        assert_eq!((p.from_level, p.level), (70, 60), "the world's level cap");
        assert!(p.rename && p.name != "Tamsin" && p.name.starts_with("Tamsin"), "the name is taken there");
        assert_eq!(p.at_login & (AT_RENAME | AT_RESET_SPELLS | AT_RESET_TALENTS), AT_RENAME | AT_RESET_SPELLS | AT_RESET_TALENTS);
        // items: the sword comes with its enchantment; potions in stacks of 5; unknown and too-high items to the vault
        let sword = p.items.iter().find(|i| i.entry == 19019).unwrap();
        assert_eq!((sword.ench.clone(), sword.durability), (vec![(0, 1900)], 125));
        assert_eq!(p.items.iter().filter(|i| i.entry == 13446).map(|i| i.n).collect::<Vec<_>>(), vec![5; 9]);
        let v: Vec<(&str, &str)> = p.vault.iter().map(|v| (v.id.as_str(), v.why.as_str())).collect();
        assert!(v.contains(&("stock-335a:item:90001", "Original world doesn't have it")));
        assert!(v.contains(&("stock-335a:item:34000", "needs level 70")));
        // gold never travels; quests it doesn't have, raid lockouts and quests in progress stay
        assert!(p.stays.iter().any(|s| s.starts_with("Gold (500 gold)")));
        assert!(p.stays.iter().any(|s| s.contains("raid lockouts")) && p.stays.iter().any(|s| s.contains("in progress")) && p.stays.iter().any(|s| s.contains("1 finished quests")));
        assert_eq!(p.done, vec![100, 200]);
        assert!(p.spells.is_empty(), "spells are learned again when the level is lowered");
        assert_eq!(p.skills, vec![(171, 300, 375)], "professions stay, the rest the world sets");
        assert!(p.comes.iter().any(|c| c.contains("mailbox")));
        // not capped: spells and every skill come
        let mut t80 = t.clone();
        t80.cap = 80;
        let p80 = customs(&r, &t80);
        assert_eq!((p80.level, p80.spells.len(), p80.skills.len()), (70, 2, 2));
        assert_eq!(p80.at_login & AT_RESET_SPELLS, 0);
        // refusals: another kind of world, no such race and class there, a full login, a tampered save
        let mut other = t.clone();
        other.ns = "other-112".into();
        assert!(!customs(&r, &other).ok);
        let mut nostart = t.clone();
        nostart.start = None;
        assert!(customs(&r, &nostart).why.contains("no Night Elf Druids"));
        let mut full = t.clone();
        full.count = 10;
        assert!(customs(&r, &full).why.contains("10 characters"));
        let mut bad = r.clone();
        bad["save"]["who"]["money"] = json!(0);
        assert!(!customs(&bad, &t).ok);
        assert_eq!(free_name("Odo", &[]), ("Odo".into(), false));
        assert_eq!(free_name("Odo", &["odo".into(), "Odoaa".into()]), ("Odoab".into(), true));
    }

    /// The rows an import's SQL writes, per table (our own SQL's shape: one INSERT per line).
    fn rows_of(sql: &str) -> BTreeMap<String, Vec<String>> {
        let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for line in sql.lines().filter(|l| l.starts_with("INSERT INTO ")) {
            let table = line["INSERT INTO ".len()..].split(' ').next().unwrap().to_string();
            let vals = &line[line.find(" VALUES ").unwrap() + 8..line.len() - 1];
            for row in vals.split("), (") {
                m.entry(table.clone()).or_default().push(row.trim_matches(|c| c == '(' || c == ')').to_string());
            }
        }
        m
    }

    #[test]
    fn a_fixture_round_trip_makes_whole_rows_and_no_gold() {
        let d = tempfile::tempdir().unwrap();
        let r = one_save(d.path(), &TestKeys { player: 1, world: 10 });
        check_chain(&[r.clone()]).unwrap();
        let t = target_for(&r);
        let p = customs(&r, &t);
        let sql = import_sql(&p, "original", t.start.as_ref().unwrap(), 5_000).unwrap();
        assert!(sql.starts_with("START TRANSACTION;") && sql.trim_end().ends_with("COMMIT;"), "one transaction");
        assert!(sql.contains("username = 'ORIGINAL'"));
        let rows = rows_of(&sql);
        let ch = &rows["characters"];
        assert_eq!(ch.len(), 1);
        let f: Vec<&str> = ch[0].split(", ").collect();
        assert_eq!(&f[..9], ["@g", "@acc", &format!("'{}'", p.name), "4", "11", "1", "60", "0", "0"], "level capped, no xp, no gold");
        assert_eq!(&f[14..20], ["1", "141", "10311.3", "832.463", "1326.41", "5.69632"], "the world's own start");
        assert_eq!(rows["item_instance"].len(), p.items.len());
        assert!(rows["item_instance"].iter().any(|r| r.contains("19019") && r.contains("'1900 0 0 0 0 0 ") && r.ends_with("125, 0, ''")));
        assert!(!rows["item_instance"].iter().any(|r| r.contains("90001") || r.contains("34000")), "vault items aren't made");
        assert_eq!(rows["mail"].len(), p.items.len().div_ceil(12));
        assert!(rows["mail"].iter().all(|m| m.contains(", 0, 0, 0") && !m.contains("money")));
        assert_eq!(rows["mail_items"].len(), p.items.len(), "every item in a letter");
        assert_eq!(rows["character_queststatus_rewarded"], vec!["@g, 100, 1", "@g, 200, 1"]);
        assert_eq!(rows["character_reputation"], vec!["@g, 72, 3000, 17"]);
        assert_eq!(rows["character_skills"], vec!["@g, 171, 300, 375"]);
        assert!(!rows.contains_key("character_spell"));
        assert_eq!(rows["character_achievement"], vec!["@g, 6, 1700000000"]);
        assert!(sql.contains("REPLACE INTO acore_auth.realmcharacters"));
        // a refused plan, a bad login or a bad start writes nothing
        let mut no = p.clone();
        no.ok = false;
        assert!(import_sql(&no, "ORIGINAL", t.start.as_ref().unwrap(), 1).is_err());
        assert!(import_sql(&p, "x'; --", t.start.as_ref().unwrap(), 1).is_err());
        let mut bad_start = t.start.clone().unwrap();
        bad_start.2 = "1; DROP".into();
        assert!(import_sql(&p, "ORIGINAL", &bad_start, 1).is_err());
    }
}
