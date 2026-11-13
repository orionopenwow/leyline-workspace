//! Leaderboards for Boss Rush and Duels, launcher side (docs/LEADERBOARDS.md): signed boards read from the website
//! and checked here, every signature, so a row shows only if the realm that recorded it signed it.
//!
//! * A realm's board (realm/custom/boards-export.py) is one document signed with the realm's own Ed25519 key, the same
//!   key style as the mesh (docs/LEYLINE-MESH.md section 3): `realm id = base32(sha256(key))[..16]`, Ed25519 over
//!   `context + "\n" + canonical JSON`. Inside, every result is signed on its own (`leyline-board-result-v1`) and
//!   names its realm; the whole document is signed too (`leyline-board-v1`), so a mirror can't drop or add a row.
//! * Where the boards come from: the site's index (`/wrath/api/boards/index.json`) lists the official realms' boards
//!   and the listed community realms' boards it has checked; each at a stable URL. A board is "official" only when
//!   the site's index says so (it came from our own site over https) or its key is pinned in [`OFFICIAL_KEYS`].
//! * Local runs (Boss Rush and single player on this PC) are never mixed in: they go to the site's separate local-run
//!   board only when the player presses Share, signed with this launcher's own key (`leyline-board-local-v1`), and
//!   show with a "not checked by a server" mark.
//!
//! The UI (ui/boards.js) ranks the rows: Boss Rush by time (each character's best), Duels by rating or this week's
//! points / wins. Nothing from a board is ever parsed as HTML.

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::p2p::mesh::{realm_id, verify_doc};

pub const CTX_RESULT: &str = "leyline-board-result-v1";
pub const CTX_BOARD: &str = "leyline-board-v1";
pub const CTX_LOCAL: &str = "leyline-board-local-v1";
pub const INDEX_PATH: &str = "/wrath/api/boards/index.json";
pub const BOARD_PREFIX: &str = "/wrath/api/boards/";
pub const LOCAL_PATH: &str = "/wrath/api/boards/local";
pub const LOCAL_JSON_PATH: &str = "/wrath/api/boards/local.json";
/// Official realms' board keys (hex Ed25519 public keys), pinned in the release so a board is "official" even when it
/// comes from a mirror. TODO(owner): the official realm's key, once Control's `realm` step has printed it
/// (`boards-export.py --whoami`).
pub const OFFICIAL_KEYS: &[&str] = &[];
pub const MAX_AGE: u64 = 400 * 86_400;
pub const SKEW: u64 = 300;
pub const MAX_RESULTS: usize = 4000;
pub const MAX_SOURCES: usize = 32;
pub const TIMEOUT: Duration = Duration::from_secs(8);
pub const CLASSES: &[&str] = &["warrior", "paladin", "hunter", "rogue", "priest", "deathknight", "shaman", "mage", "warlock", "druid"];
const DUEL_BOARDS: &[&str] = &["duel", "arena-1v1"];

/// The site, or LEYLINE_BOARDS_SITE for a test server (https, or http on this PC only).
pub fn site() -> String {
    match std::env::var("LEYLINE_BOARDS_SITE") {
        Ok(b) if b.starts_with("https://") || b.starts_with("http://127.0.0.1:") || b.starts_with("http://localhost:") => {
            b.trim_end_matches('/').to_string()
        }
        _ => crate::site!("").to_string(),
    }
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// A character name the boards show: 2-12 plain letters, no brand word.
pub fn name_ok(s: &str) -> bool {
    (2..=12).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic()) && crate::host::bossrush::brand_free(s)
}

fn realm_id_ok(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
}

fn key_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !s.starts_with('-')
}

fn uint(v: Option<&Value>, max: u64) -> Option<u64> {
    v.and_then(Value::as_u64).filter(|n| *n <= max)
}

fn class_of(v: Option<&Value>) -> &'static str {
    let c = v.and_then(Value::as_str).unwrap_or("");
    CLASSES.iter().find(|x| **x == c).copied().unwrap_or("unknown")
}

/// A realm's name as the board shows it: no control characters or angle brackets, 40 characters, no brand word.
pub fn realm_name(v: Option<&Value>) -> String {
    let s: String = v.and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control() && *c != '<' && *c != '>').take(40).collect();
    let s = s.trim().to_string();
    if s.is_empty() || !crate::host::bossrush::brand_free(&s) {
        "A realm".into()
    } else {
        s
    }
}

/// Only the fields a board shows, each checked (the same rules as stream/boards.py `clean_result`); anything else in
/// a signed record is ignored. None: the row is not shown.
pub fn clean_result(r: &Value) -> Option<Value> {
    if r.get("v").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let name = r.get("name").and_then(Value::as_str).filter(|n| name_ok(n))?;
    let at = uint(r.get("at"), u32::MAX as u64)?;
    let class = class_of(r.get("class"));
    match r.get("mode").and_then(Value::as_str) {
        Some("bossrush") => {
            let board = r.get("board").and_then(Value::as_str).filter(|b| key_ok(b))?;
            let diff = r.get("diff").and_then(Value::as_str).filter(|d| *d == "normal" || *d == "heroic")?;
            let ms = uint(r.get("ms"), 6 * 3_600_000).filter(|m| *m > 0)?;
            let party: Vec<Value> = r
                .get("party")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .take(40)
                        .filter(|p| p.is_object())
                        .map(|p| {
                            let n = p.get("name").and_then(Value::as_str).filter(|n| name_ok(n));
                            json!({"name": n, "class": class_of(p.get("class")), "bot": p.get("bot").and_then(Value::as_bool) == Some(true)})
                        })
                        .collect()
                })
                .unwrap_or_default();
            let kind = if r.get("kind").and_then(Value::as_str) == Some("group") { "group" } else { "bots" };
            let size = uint(r.get("size"), 40).unwrap_or(party.len() as u64);
            let mut out = json!({
                "mode": "bossrush", "board": board, "diff": diff, "kind": kind,
                "name": name, "class": class, "ms": ms,
                "size": size,
                "bots": uint(r.get("bots"), 40).unwrap_or(0),
                "party": party, "at": at, "run": uint(r.get("run"), u32::MAX as u64).unwrap_or(0),
            });
            if let Some(d) = uint(r.get("deaths"), 10_000) {
                out["deaths"] = json!(d);
            }
            Some(out)
        }
        Some("duels") => {
            let board = r.get("board").and_then(Value::as_str).filter(|b| DUEL_BOARDS.contains(b))?;
            let period = r.get("period").and_then(Value::as_str).filter(|p| {
                *p == "all"
                    || (p.len() == 15 && p.starts_with("week:") && p[5..].bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() }))
            })?;
            Some(json!({
                "mode": "duels", "board": board, "period": period, "name": name, "class": class,
                "rating": uint(r.get("rating"), 100_000).unwrap_or(0), "wins": uint(r.get("wins"), u32::MAX as u64).unwrap_or(0),
                "losses": uint(r.get("losses"), u32::MAX as u64).unwrap_or(0), "points": uint(r.get("points"), u32::MAX as u64).unwrap_or(0),
                "at": at,
            }))
        }
        _ => None,
    }
}

/// A board that checked: the realm, its key, its name and its rows (cleaned).
#[derive(Debug, Clone)]
pub struct Board {
    pub realm: String,
    pub pubkey: String,
    pub name: String,
    pub ts: u64,
    pub featured: Option<String>,
    pub rows: Vec<Value>,
}

/// Check a signed board as every reader does (boards-export.py `check_board`): the key makes the id, the document
/// and every result are signed by it, every result names this realm, not older than [`MAX_AGE`]. One bad result
/// refuses the whole board.
pub fn check_board(board: &Value, now: u64) -> Result<Board, String> {
    let doc = board.get("doc").filter(|d| d.is_object()).ok_or("shape")?;
    let pk = doc.get("pubkey").and_then(Value::as_str).filter(|k| hex64(k)).ok_or("pubkey")?;
    let realm = doc.get("realm").and_then(Value::as_str).ok_or("realm id")?;
    if realm_id(&unhex(pk).ok_or("pubkey")?) != realm {
        return Err("realm id".into());
    }
    let sig = board.get("sig").and_then(Value::as_str).ok_or("signature")?;
    if !verify_doc(pk, doc, CTX_BOARD, sig) {
        return Err("signature".into());
    }
    let ts = doc.get("ts").and_then(Value::as_u64).ok_or("expired")?;
    if ts > now + SKEW || now.saturating_sub(ts) > MAX_AGE {
        return Err("expired".into());
    }
    let results = doc.get("results").and_then(Value::as_array).filter(|a| a.len() <= MAX_RESULTS).ok_or("results")?;
    let mut rows = Vec::new();
    for x in results {
        let r = x.get("r").filter(|r| r.is_object()).ok_or("result shape")?;
        if r.get("realm").and_then(Value::as_str) != Some(realm) {
            return Err("result realm".into());
        }
        let s = x.get("sig").and_then(Value::as_str).ok_or("result signature")?;
        if !verify_doc(pk, r, CTX_RESULT, s) {
            return Err("result signature".into());
        }
        if let Some(c) = clean_result(r) {
            rows.push(c);
        }
    }
    let featured = doc
        .get("featured")
        .and_then(|f| f.get("bossrush"))
        .and_then(Value::as_str)
        .filter(|k| crate::host::bossrush::fight(k).is_some())
        .map(String::from);
    Ok(Board { realm: realm.to_string(), pubkey: pk.to_string(), name: realm_name(doc.get("name")), ts, featured, rows })
}

/// The local-run board (the site's, labelled): rows with only the shown fields, `local: true`.
pub fn local_rows(doc: &Value) -> Vec<Value> {
    let Some(runs) = doc.get("runs").and_then(Value::as_array) else { return Vec::new() };
    runs.iter()
        .take(5000)
        .filter_map(|x| {
            let board = x.get("board").and_then(Value::as_str).filter(|b| crate::host::bossrush::fight(b).is_some())?;
            let diff = x.get("diff").and_then(Value::as_str).filter(|d| *d == "normal" || *d == "heroic")?;
            let engine = x.get("engine").and_then(Value::as_str).filter(|e| *e == "exact" || *e == "light")?;
            let name = x.get("name").and_then(Value::as_str).filter(|n| name_ok(n))?;
            let ms = uint(x.get("ms"), 6 * 3_600_000).filter(|m| *m > 0)?;
            Some(json!({"mode": "bossrush", "board": board, "diff": diff, "engine": engine, "name": name,
                "class": class_of(x.get("class")), "ms": ms, "at": uint(x.get("at"), u32::MAX as u64).unwrap_or(0), "local": true}))
        })
        .collect()
}

/// Every board the index lists, fetched with `fetch` (a site path to JSON), checked here, as one view for the UI:
/// `{week, featured, realms: [{id, name, kind, ts}], rows: [... with realm {id, name, kind, signed: true}],
/// local: [...], skipped}`. A board that fails a check is skipped (counted), never shown.
pub fn gather(index: &Value, fetch: &dyn Fn(&str) -> Result<Value, String>, now: u64, pinned: &[&str]) -> Value {
    let mut realms = Vec::new();
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut skipped = 0u64;
    let mut featured: Option<String> = None;
    let mut sources: Vec<(&str, &Value)> = Vec::new();
    for kind in ["official", "community"] {
        if let Some(a) = index.get(kind).and_then(Value::as_array) {
            sources.extend(a.iter().map(|e| (kind, e)));
        }
    }
    for (listed_kind, entry) in sources.into_iter().take(MAX_SOURCES) {
        let Some(rid) = entry.get("realm").and_then(Value::as_str).filter(|r| realm_id_ok(r)) else { skipped += 1; continue };
        if !seen.insert(rid.to_string()) {
            continue;
        }
        let path = format!("{BOARD_PREFIX}{rid}.json");
        let board = match fetch(&path).and_then(|b| check_board(&b, now)) {
            Ok(b) if b.realm == rid && entry.get("pubkey").and_then(Value::as_str).map_or(true, |k| k == b.pubkey) => b,
            _ => {
                skipped += 1;
                continue;
            }
        };
        let kind = if listed_kind == "official" || pinned.contains(&board.pubkey.as_str()) { "official" } else { "community" };
        if kind == "official" && featured.is_none() {
            featured = board.featured.clone();
        }
        let realm = json!({"id": board.realm, "name": board.name, "kind": kind, "signed": true});
        realms.push(json!({"id": board.realm, "name": board.name, "kind": kind, "ts": board.ts}));
        for mut r in board.rows {
            r["realm"] = realm.clone();
            rows.push(r);
        }
    }
    let local = index
        .get("local")
        .and_then(Value::as_str)
        .filter(|p| *p == LOCAL_JSON_PATH)
        .and_then(|p| fetch(p).ok())
        .map(|d| local_rows(&d))
        .unwrap_or_default();
    let week = crate::host::bossrush::week_start(now);
    json!({
        "week": week,
        "now": now,
        "featured": featured.unwrap_or_else(|| crate::host::bossrush::featured_fight(now).key.to_string()),
        "featured_local": crate::host::bossrush::featured_fight(now).key,
        "realms": realms,
        "rows": rows,
        "local": local,
        "skipped": skipped,
    })
}

/// A run on this PC, signed with this launcher's own key for the local-run board (`leyline-board-local-v1`). The
/// site keeps it apart from the signed boards and says it wasn't checked by a server.
#[allow(clippy::too_many_arguments)]
pub fn local_record(me: &crate::identity::PeerIdentity, fight: &str, diff: &str, engine: &str, name: &str, class: &str, spec: &str,
    ms: u64, at: u64) -> Result<Value, String> {
    if crate::host::bossrush::fight(fight).is_none() {
        return Err("That fight isn't on the boards.".into());
    }
    if diff != "normal" && diff != "heroic" {
        return Err("Pick Normal or Heroic.".into());
    }
    if engine != "exact" && engine != "light" {
        return Err("That run's engine isn't known.".into());
    }
    if !name_ok(name) {
        return Err("Pick a name of 2 to 12 letters for the board.".into());
    }
    if !(10_000..=6 * 3_600_000).contains(&ms) {
        return Err("That run's time doesn't look right.".into());
    }
    let spec: String = spec.chars().filter(|c| c.is_ascii_lowercase()).take(12).collect();
    let r = json!({"v": 1, "peer": me.peer_id, "mode": "bossrush", "board": fight, "diff": diff, "engine": engine,
        "name": name, "class": class_of(Some(&json!(class))), "spec": spec, "ms": ms, "at": at});
    Ok(json!({"pubkey": me.public_hex(), "sig": me.sign_doc(CTX_LOCAL, &r), "r": r}))
}

fn cache_path(app: &Path) -> PathBuf {
    app.join("cache").join("boards.json")
}

/// The boards for the Leaderboards view: fetched and checked now, else the last view this PC saw (`from_cache`).
pub fn load(app: &Path) -> Value {
    let base = site();
    let fetch = |path: &str| crate::net::get_json(&format!("{base}{path}"), TIMEOUT);
    let now = crate::p2p::mesh::now_secs();
    match fetch(INDEX_PATH) {
        Ok(index) => {
            let mut v = gather(&index, &fetch, now, OFFICIAL_KEYS);
            let _ = std::fs::create_dir_all(app.join("cache"));
            let _ = crate::settings::write_atomic(&cache_path(app), v.to_string().as_bytes());
            v["from_cache"] = json!(false);
            v
        }
        Err(e) => {
            let cached = std::fs::read_to_string(cache_path(app)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
            match cached {
                Some(mut v) if v.is_object() => {
                    v["from_cache"] = json!(true);
                    v
                }
                _ => {
                    let mut m = Map::new();
                    m.insert("error".into(), json!(format!("The boards aren't reachable right now ({}).", e.chars().take(120).collect::<String>())));
                    m.insert("rows".into(), json!([]));
                    m.insert("local".into(), json!([]));
                    m.insert("realms".into(), json!([]));
                    m.insert("now".into(), json!(now));
                    m.insert("week".into(), json!(crate::host::bossrush::week_start(now)));
                    m.insert("featured".into(), json!(crate::host::bossrush::featured_fight(now).key));
                    m.insert("featured_local".into(), json!(crate::host::bossrush::featured_fight(now).key));
                    Value::Object(m)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::flags::canonical;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    const VECTOR: &str = include_str!("boards-vector.json");
    const VTS: u64 = 1_794_510_000;

    struct Key(Ed25519KeyPair);
    impl Key {
        fn new(seed: u8) -> Key {
            Key(Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap())
        }
        fn pubhex(&self) -> String {
            crate::net::hex(self.0.public_key().as_ref())
        }
        fn rid(&self) -> String {
            realm_id(self.0.public_key().as_ref())
        }
        fn sign(&self, ctx: &str, doc: &Value) -> String {
            crate::net::hex(self.0.sign(format!("{ctx}\n{}", canonical(doc)).as_bytes()).as_ref())
        }
        fn board(&self, name: &str, ts: u64, results: Vec<Value>) -> Value {
            let signed: Vec<Value> = results
                .into_iter()
                .map(|mut r| {
                    r["realm"] = json!(self.rid());
                    json!({"sig": self.sign(CTX_RESULT, &r), "r": r})
                })
                .collect();
            let doc = json!({"v": 1, "realm": self.rid(), "pubkey": self.pubhex(), "name": name, "ts": ts, "seq": ts * 1000,
                "week": "2026-11-09", "featured": {"bossrush": "frozen-throne"}, "results": signed});
            json!({"sig": self.sign(CTX_BOARD, &doc), "doc": doc})
        }
    }

    fn clear(name: &str, ms: u64, at: u64) -> Value {
        json!({"v": 1, "realm": "", "mode": "bossrush", "board": "frozen-throne", "diff": "normal", "kind": "bots", "name": name,
            "class": "mage", "ms": ms, "size": 10, "bots": 9, "party": [], "at": at, "run": ms})
    }

    #[test]
    fn python_signed_boards_verify_here() {
        let v: Value = serde_json::from_str(VECTOR).unwrap();
        let b = check_board(&v, VTS + 10).unwrap();
        assert_eq!(b.realm, "72asyextvngonlc5");
        assert_eq!(b.name, "Fröst Håven \"Q\"");
        assert_eq!(b.rows.len(), 4);
        assert_eq!(b.rows[0]["deaths"], json!(1));
        assert_eq!(b.rows[0]["party"][1], json!({"name": null, "class": "priest", "bot": true}));
        assert!(b.rows[1].get("deaths").is_none(), "no deaths counted: none shown");
        assert_eq!(b.rows[2]["rating"], json!(1712));
        assert_eq!(b.rows[3]["period"], json!("week:2026-11-09"));
        // the featured fight: the same rule here and in the export
        assert_eq!(b.featured.as_deref(), Some(crate::host::bossrush::featured_fight(VTS).key));
    }

    #[test]
    fn tampering_is_refused() {
        let v: Value = serde_json::from_str(VECTOR).unwrap();
        let mut t = v.clone();
        t["doc"]["results"][0]["r"]["ms"] = json!(1);
        assert_eq!(check_board(&t, VTS).unwrap_err(), "signature", "the document signs every result");
        let mut t = v.clone();
        t["doc"]["results"].as_array_mut().unwrap().pop();
        assert_eq!(check_board(&t, VTS).unwrap_err(), "signature", "a dropped row");
        let mut t = v.clone();
        t["doc"]["realm"] = json!("aaaaaaaaaaaaaaaa");
        assert_eq!(check_board(&t, VTS).unwrap_err(), "realm id");
        assert_eq!(check_board(&v, VTS + MAX_AGE + 1).unwrap_err(), "expired");
        assert_eq!(check_board(&json!({"doc": 1}), VTS).unwrap_err(), "shape");

        // another key can't sign for this realm, nor slip its own row in
        let (a, b) = (Key::new(1), Key::new(2));
        let good = a.board("Alpha", VTS, vec![clear("Orion", 300_000, VTS)]);
        assert!(check_board(&good, VTS).is_ok());
        let mut t = good.clone();
        t["doc"]["pubkey"] = json!(b.pubhex());
        assert_eq!(check_board(&t, VTS).unwrap_err(), "realm id");
        let other = b.board("Beta", VTS, vec![clear("Cheat", 1000, VTS)]);
        let mut doc = good["doc"].clone();
        doc["results"].as_array_mut().unwrap().push(other["doc"]["results"][0].clone());
        let mixed = json!({"sig": a.sign(CTX_BOARD, &doc), "doc": doc});
        assert_eq!(check_board(&mixed, VTS).unwrap_err(), "result realm");
        // a result signed with the board's key but under the wrong context
        let mut r = clear("Orion", 1000, VTS);
        r["realm"] = json!(a.rid());
        let mut doc = good["doc"].clone();
        doc["results"] = json!([{"r": r.clone(), "sig": a.sign(CTX_BOARD, &r)}]);
        let wrong = json!({"sig": a.sign(CTX_BOARD, &doc), "doc": doc});
        assert_eq!(check_board(&wrong, VTS).unwrap_err(), "result signature");
    }

    #[test]
    fn rows_show_only_checked_fields() {
        let mut r = clear("Orion", 300_000, VTS);
        r["guid"] = json!(77);
        r["class"] = json!("<b>");
        let c = clean_result(&r).unwrap();
        assert!(c.get("guid").is_none());
        assert_eq!(c["class"], json!("unknown"));
        for bad in ["Blizzard", "<script>", "A", "Thirteenchars", "wow"] {
            assert!(clean_result(&clear(bad, 1, VTS)).is_none(), "{bad}");
        }
        assert!(clean_result(&clear("Orion", 0, VTS)).is_none(), "no time");
        assert!(clean_result(&clear("Orion", 7 * 3_600_000, VTS)).is_none(), "six hours at most");
        let mut d = json!({"v": 1, "mode": "duels", "board": "arena-1v1", "period": "week:2026-11-09", "name": "Jaina", "class": "mage",
            "rating": 0, "wins": 3, "losses": 1, "points": 0, "at": VTS});
        assert!(clean_result(&d).is_some());
        d["period"] = json!("week:2026-1-09");
        assert!(clean_result(&d).is_none());
        d["period"] = json!("all");
        d["board"] = json!("3v3");
        assert!(clean_result(&d).is_none());
        assert_eq!(realm_name(Some(&json!("<Warcraft> realm"))), "A realm");
        assert_eq!(realm_name(Some(&json!("Frost <b>Haven"))), "Frost bHaven");
    }

    #[test]
    fn gather_checks_every_board_and_marks_its_realm() {
        let (a, b, c) = (Key::new(1), Key::new(2), Key::new(3));
        let off = a.board("Wrath Unleashed", VTS, vec![clear("Orion", 300_000, VTS), clear("Jaina", 280_000, VTS)]);
        let com = b.board("Frost Haven", VTS, vec![clear("Valeera", 310_000, VTS)]);
        let mut bad = c.board("Fake", VTS, vec![clear("Cheat", 1000, VTS)]);
        bad["doc"]["results"][0]["r"]["ms"] = json!(1);
        let index = json!({"v": 1, "official": [{"realm": a.rid(), "pubkey": a.pubhex()}],
            "community": [{"realm": b.rid(), "pubkey": b.pubhex()}, {"realm": c.rid()}, {"realm": "../../x"}, {"realm": a.rid()}],
            "local": LOCAL_JSON_PATH});
        let (ar, br, cr) = (a.rid(), b.rid(), c.rid());
        let fetch = move |p: &str| -> Result<Value, String> {
            if p == format!("{BOARD_PREFIX}{ar}.json") {
                Ok(off.clone())
            } else if p == format!("{BOARD_PREFIX}{br}.json") {
                Ok(com.clone())
            } else if p == format!("{BOARD_PREFIX}{cr}.json") {
                Ok(bad.clone())
            } else if p == LOCAL_JSON_PATH {
                Ok(json!({"local": true, "runs": [
                    {"board": "bone-lord", "diff": "normal", "engine": "light", "name": "Orion", "class": "mage", "ms": 245000, "at": VTS},
                    {"board": "nope", "diff": "normal", "engine": "light", "name": "Orion", "class": "mage", "ms": 245000, "at": VTS}]}))
            } else {
                Err("not found".into())
            }
        };
        let v = gather(&index, &fetch, VTS, &[]);
        let rows = v["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3, "the tampered board's row never shows");
        assert_eq!(v["skipped"], json!(2), "the tampered board and the odd id");
        assert_eq!(rows[0]["realm"], json!({"id": a.rid(), "name": "Wrath Unleashed", "kind": "official", "signed": true}));
        assert_eq!(rows[2]["realm"]["kind"], json!("community"));
        assert_eq!(v["featured"], json!("frozen-throne"));
        assert_eq!(v["local"].as_array().unwrap().len(), 1);
        assert_eq!(v["local"][0]["local"], json!(true));
        // a pinned key makes a board official wherever it is listed
        let index2 = json!({"official": [], "community": [{"realm": b.rid()}]});
        let v2 = gather(&index2, &fetch, VTS, &[b.pubhex().as_str()]);
        assert_eq!(v2["rows"][0]["realm"]["kind"], json!("official"));
        // a listed key that doesn't match the board's is skipped
        let index3 = json!({"official": [{"realm": a.rid(), "pubkey": b.pubhex()}]});
        assert_eq!(gather(&index3, &fetch, VTS, &[])["rows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn local_runs_are_signed_by_this_launcher() {
        let me = crate::identity::PeerIdentity::from_seed([9; 32]).unwrap();
        let rec = local_record(&me, "bone-lord", "normal", "light", "Orion", "deathknight", "blood", 245_000, VTS).unwrap();
        let r = &rec["r"];
        assert_eq!(r["peer"], json!(me.peer_id));
        assert_eq!(r["engine"], json!("light"));
        assert!(verify_doc(rec["pubkey"].as_str().unwrap(), r, CTX_LOCAL, rec["sig"].as_str().unwrap()));
        assert!(!verify_doc(rec["pubkey"].as_str().unwrap(), r, CTX_RESULT, rec["sig"].as_str().unwrap()), "never a realm's result");
        assert!(local_record(&me, "nope", "normal", "light", "Orion", "mage", "", 245_000, VTS).is_err());
        assert!(local_record(&me, "bone-lord", "normal", "light", "Blizzard", "mage", "", 245_000, VTS).is_err());
        assert!(local_record(&me, "bone-lord", "mythic", "light", "Orion", "mage", "", 245_000, VTS).is_err());
        assert!(local_record(&me, "bone-lord", "normal", "light", "Orion", "mage", "", 500, VTS).is_err());
    }
}
