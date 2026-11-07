//! Leyline Solo's fast sign-in (docs/LEYLINE-SOLO.md, Phase 2): the player's own first-era game folder pointed at
//! this PC's world and, in the opt-in "addon" mode, our sign-in add-on in a patch archive of ours. Ported from
//! Phase 0's tools/solo/solo_proof.py (`signin`, `signin --off`).
//!
//! Every file it changes is backed up first (once, into the install's `signin-backup/`) and [`restore`] puts it back
//! byte for byte with its old modified time, or removes it when it wasn't there. Two modes ([`Mode`]):
//!
//! * **prefill** (the default): `realmlist.wtf` = this PC, `WTF/Config.wtf` = the account name `SOLO` and the realm
//!   "Leyline Solo". The player types the password once (the UI shows it on request).
//! * **addon** (opt-in, `TODO(owner)`: copying the player's own login-screen file list into our archive isn't approved
//!   yet): the same, plus `Data/patch-Z.MPQ` holding the login screen's own `GlueXML.toc` as the player's archives have
//!   it with one line added (`LeylineSolo.xml`), our `LeylineSolo.xml`/`.lua` (`launcher/src-tauri/glue/`), and
//!   `LeylineSoloSession.lua`: the sign-in for this PC's world, every byte a `\ddd` escape. The password is taken out
//!   of it again ([`scrub_session`]) once the game lets go of the archive, and on stop.
//!
//! An archive in that slot that Leyline didn't write is never touched. No game file is copied: the file list stays
//! on the player's PC, inside the player's own folder.

use crate::hybrid::{char_name_ok, NewChar};
use crate::mpq;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SLOT: &str = "patch-Z.MPQ";
pub const OUR_LUA: &str = "Interface\\GlueXML\\LeylineSolo.lua";
pub const OUR_XML: &str = "Interface\\GlueXML\\LeylineSolo.xml";
pub const SESSION: &str = "Interface\\GlueXML\\LeylineSoloSession.lua";
pub const TOC: &str = "Interface\\GlueXML\\GlueXML.toc";
/// Ours is a few KB; anything bigger in the slot is someone else's.
const OURS_MAX: u64 = 4 * 1024 * 1024;
pub const NO_SESSION: &str = "-- Leyline Solo: no sign-in in here now (the launcher writes one for a start and takes it out after).\r\nLEYLINE_SOLO_SESSION = nil\r\n";
const GLUE_LUA: &[u8] = include_bytes!("../../glue/LeylineSolo.lua");
const GLUE_XML: &[u8] = include_bytes!("../../glue/LeylineSolo.xml");
/// The account and the realm the sign-in fills in (host/classic.rs makes them).
pub const ACCOUNT: &str = "SOLO";
/// TODO(owner): the name "Leyline Solo" on the realm list and in the add-on's line.
pub const REALM: &str = "Leyline Solo";
pub const REALMLIST: &[u8] = b"set realmlist 127.0.0.1\r\n";

/// The setting `solo_signin` (settings.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Prefill,
    Addon,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Prefill => "prefill",
            Mode::Addon => "addon",
        }
    }
    /// The page's word: "simple" (prefill) or "quick" (the add-on).
    pub fn ui_word(self) -> &'static str {
        match self {
            Mode::Prefill => "simple",
            Mode::Addon => "quick",
        }
    }
    /// The setting's word ("addon" or the page's "quick"); anything else is the default (prefill).
    pub fn from_word(w: &str) -> Mode {
        if w.trim().eq_ignore_ascii_case("addon") || w.trim().eq_ignore_ascii_case("quick") {
            Mode::Addon
        } else {
            Mode::Prefill
        }
    }
    pub fn parse(w: &str) -> Result<Mode, String> {
        match w.trim().to_ascii_lowercase().as_str() {
            "prefill" | "simple" => Ok(Mode::Prefill),
            "addon" | "quick" => Ok(Mode::Addon),
            _ => Err("The sign-in is either \"simple\" or \"quick\".".into()),
        }
    }
}

/// One start's sign-in for the add-on.
#[derive(Clone, PartialEq)]
pub struct Session {
    pub user: String,
    pub pass: String,
    pub realm: String,
    /// Enter the world with this character once character select is up.
    pub pick: Option<String>,
    /// Make this character on the create screen first, then enter it (Quick start).
    pub make: Option<NewChar>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("user", &self.user).field("pass", &"[hidden]").field("realm", &self.realm).field("pick", &self.pick).field("make", &self.make).finish()
    }
}

pub fn lua_bytes(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 4 + 2);
    o.push('"');
    for b in s.bytes() {
        o.push_str(&format!("\\{b:03}"));
    }
    o.push('"');
    o
}

fn file_word_ok(w: &str) -> bool {
    (2..=16).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_alphabetic())
}

/// `LeylineSoloSession.lua` for this sign-in (or the empty one): every value as escaped bytes. Only the shapes the
/// first-era game takes: an account and password of 1 to 16 upper-case letters and digits, a realm name of letters,
/// digits and spaces, a character's name, a race's and class's file names.
pub fn session_lua(s: Option<&Session>) -> Result<String, String> {
    let Some(s) = s else { return Ok(NO_SESSION.to_string()) };
    let shape = |v: &str| (1..=16).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if !shape(&s.user) || !shape(&s.pass) {
        return Err("That sign-in can't be handed to the game.".into());
    }
    if !(1..=32).contains(&s.realm.len()) || !s.realm.bytes().all(|b| b.is_ascii_alphanumeric() || b == b' ') {
        return Err("That realm name can't be handed to the game.".into());
    }
    let pick = match &s.pick {
        Some(n) if char_name_ok(n) => format!(", pick = {}", lua_bytes(n)),
        Some(_) => return Err("That character's name can't be handed to the game.".into()),
        None => String::new(),
    };
    let make = match &s.make {
        Some(m) if char_name_ok(&m.name) && file_word_ok(&m.race) && file_word_ok(&m.class) => format!(
            ", make = {{ name = {}, race = {}, class = {}, female = {} }}",
            lua_bytes(&m.name),
            lua_bytes(&m.race),
            lua_bytes(&m.class),
            m.female
        ),
        Some(_) => return Err("That new character can't be handed to the game.".into()),
        None => String::new(),
    };
    Ok(format!(
        "-- Leyline Solo: the sign-in to this PC's own world for one start, written by the launcher and taken out after.\r\n\
         LEYLINE_SOLO_SESSION = {{ user = {}, pass = {}, realm = {}{pick}{make} }}\r\n",
        lua_bytes(&s.user),
        lua_bytes(&s.pass),
        lua_bytes(&s.realm)
    ))
}

// ------------------------------------------------------------------------------------------------ the files

/// Latin-1 text of bytes and back (the game's own files are bytes; nothing is lost either way).
fn latin1(b: &[u8]) -> String {
    b.iter().map(|c| *c as char).collect()
}

fn unlatin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u32 as u8).collect()
}

/// Config.wtf with `SET key "value"` (replaced where it is, every time it is there, else added), its line ends kept.
pub fn set_wtf_line(text: &str, key: &str, value: &str) -> String {
    let nl = if text.contains("\r\n") || text.is_empty() { "\r\n" } else { "\n" };
    let new = format!("SET {key} \"{value}\"");
    let mut done = false;
    let mut lines: Vec<String> = text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect();
    for l in lines.iter_mut() {
        if is_key(l, key) {
            *l = new.clone();
            done = true;
        }
    }
    if !done {
        lines.push(new);
    }
    lines.join(nl) + nl
}

/// The same without that key's line.
pub fn remove_wtf_line(text: &str, key: &str) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !is_key(l, key)).collect();
    if lines.is_empty() {
        String::new()
    } else {
        lines.join(nl) + nl
    }
}

fn is_key(line: &str, key: &str) -> bool {
    let mut w = line.split_whitespace();
    w.next().is_some_and(|s| s.eq_ignore_ascii_case("SET")) && w.next().is_some_and(|k| k.eq_ignore_ascii_case(key))
}

/// The value of `SET key "value"` (the first one), when it is there.
pub fn wtf_value(text: &str, key: &str) -> Option<String> {
    let l = text.lines().find(|l| is_key(l, key))?;
    let rest = l.trim().splitn(3, char::is_whitespace).nth(2)?.trim();
    Some(rest.trim_matches('"').to_string())
}

/// The login screen's file list with `LeylineSolo.xml` added at its end (once), CRLF line ends as the game's own.
pub fn toc_with_ours(toc: &str) -> String {
    let mut lines: Vec<&str> = toc.lines().map(|l| l.trim_end_matches('\r')).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if !lines.iter().any(|l| l.trim().eq_ignore_ascii_case("LeylineSolo.xml")) {
        lines.push("LeylineSolo.xml");
    }
    lines.join("\r\n") + "\r\n"
}

/// The player's archives that can hold the login screen's file list, the one the game reads last first: the patches
/// (`patch-<letter>` after `patch-<digit>` after `patch`), then `interface.MPQ`; ours is never one.
pub fn slot_order(names: &[String], ours: &str) -> Vec<String> {
    let prio = |n: &str| -> (u8, u8, String) {
        let l = n.to_ascii_lowercase();
        if l == "patch.mpq" {
            return (2, 0, String::new());
        }
        if let Some(c) = l.strip_prefix("patch-").and_then(|r| r.strip_suffix(".mpq")).filter(|c| c.chars().count() == 1) {
            let ch = c.chars().next().unwrap_or('0');
            return (2, if ch.is_ascii_digit() { 1 } else { 2 }, c.to_string());
        }
        (if l == "interface.mpq" { 1 } else { 0 }, 0, l)
    };
    let mut v: Vec<String> = names
        .iter()
        .filter(|n| !n.eq_ignore_ascii_case(ours))
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l == "interface.mpq" || l == "patch.mpq" || (l.starts_with("patch-") && l.ends_with(".mpq") && l.len() == "patch-x.mpq".len())
        })
        .cloned()
        .collect();
    v.sort_by_key(|n| std::cmp::Reverse(prio(n)));
    v
}

fn data_dir(game: &Path) -> Option<PathBuf> {
    crate::clients::find_ci(game, "Data").filter(|d| d.is_dir())
}

fn mpq_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.to_ascii_lowercase().ends_with(".mpq")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// `Interface\GlueXML\GlueXML.toc` as the player's own archives hold it (the one the game reads last wins) and which
/// archive it came from; or why it can't be read here.
pub fn player_toc(game: &Path, slot: &str) -> Result<(String, String), String> {
    let d = data_dir(game).ok_or("no Data folder")?;
    let mut why = "no archive in your Data folder holds the login screen's file list".to_string();
    for n in slot_order(&mpq_names(&d), slot) {
        match mpq::GameArchive::open(&d.join(&n)).and_then(|mut a| a.read(TOC)) {
            Ok(Some(b)) if b.len() <= 64 * 1024 => return Ok((latin1(&b), n)),
            Ok(_) => {}
            Err(e) => why = format!("{n} couldn't be read here ({e})"),
        }
    }
    Err(why)
}

/// Our archive: small, with our add-on inside.
pub fn is_ours(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.len() <= OURS_MAX).unwrap_or(false) && fs::read(p).ok().is_some_and(|b| mpq::read(&b, OUR_LUA).is_some())
}

/// The archive's bytes: the file list with our line, our two files, the session.
pub fn archive(toc_with_line: &str, session: &str) -> Result<Vec<u8>, String> {
    mpq::build(&[
        (TOC.to_string(), unlatin1(toc_with_line)),
        (OUR_XML.to_string(), GLUE_XML.to_vec()),
        (OUR_LUA.to_string(), GLUE_LUA.to_vec()),
        (SESSION.to_string(), session.as_bytes().to_vec()),
    ])
}

fn sha(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn write_file(p: &Path, b: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(p, b).map_err(|e| format!("Couldn't write {} (is the game open? close it first): {e}", p.display()))
}

fn mtime(p: &Path) -> Option<SystemTime> {
    fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn set_mtime(p: &Path, t: SystemTime) {
    if let Ok(f) = fs::OpenOptions::new().write(true).open(p) {
        let _ = f.set_modified(t);
    }
}

// ------------------------------------------------------------------------------------------------ on and off

/// What is on in a game folder, kept in the install's state.json.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SignState {
    pub on: bool,
    pub game: String,
    pub mode: String,
    /// A file we changed (relative to the game folder) -> its backup's name, or "" when it wasn't there before.
    pub changed: BTreeMap<String, String>,
    /// The sha256 of what we last wrote there (the game may rewrite Config.wtf after).
    pub wrote: BTreeMap<String, String>,
    /// A file we added (our archive) -> the sha256 of what we wrote.
    pub added: BTreeMap<String, String>,
    /// Config.wtf's values before ours (None: the line wasn't there).
    pub cfg_old: BTreeMap<String, Option<String>>,
    /// Why the add-on isn't in (addon mode only; empty when it is).
    pub why: String,
}

/// What [`install`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// The add-on is in with this session.
    pub addon: bool,
    /// Why not (addon mode), in a sentence.
    pub why: String,
    pub lines: Vec<String>,
}

fn keep_original(game: &Path, bdir: &Path, st: &mut SignState, rel: &str) -> Result<(), String> {
    if st.changed.contains_key(rel) || st.added.contains_key(rel) {
        return Ok(());
    }
    let p = game.join(rel);
    if p.is_file() {
        fs::create_dir_all(bdir).map_err(|e| e.to_string())?;
        let name = format!("{:02}-{}", st.changed.len() + 1, rel.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '_' }).collect::<String>());
        fs::copy(&p, bdir.join(&name)).map_err(|e| format!("Couldn't back up {rel}: {e}"))?;
        if let Some(t) = mtime(&p) {
            set_mtime(&bdir.join(&name), t);
        }
        st.changed.insert(rel.to_string(), name);
    } else {
        st.changed.insert(rel.to_string(), String::new());
    }
    Ok(())
}

/// The file `name` under `dir` as it is spelled there (any case), else `name` itself.
fn rel_ci(game: &Path, dir: Option<&str>, name: &str) -> String {
    let base = match dir {
        Some(d) => match crate::clients::find_ci(game, d) {
            Some(p) => p,
            None => return format!("{d}/{name}"),
        },
        None => game.to_path_buf(),
    };
    let dname = dir.map(|_| base.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let fname = crate::clients::find_ci(&base, name).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).unwrap_or_else(|| name.to_string());
    match dname {
        Some(d) => format!("{d}/{fname}"),
        None => fname,
    }
}

/// Sign-in on for this game folder: realmlist.wtf and Config.wtf (backed up first), and in addon mode our archive
/// with `session`. Another folder that has it on is refused (turn it off there first).
pub fn install(game: &Path, bdir: &Path, st: &mut SignState, mode: Mode, session: Option<&Session>) -> Result<Installed, String> {
    if st.on && !st.game.is_empty() && Path::new(&st.game) != game {
        return Err("The Solo sign-in is on for another game folder: turn it off there first (Sign-in off).".into());
    }
    let mut out = Installed::default();
    st.on = true;
    st.game = game.display().to_string();
    st.mode = mode.word().into();
    // 1. realmlist.wtf -> this PC
    let rl = rel_ci(game, None, "realmlist.wtf");
    keep_original(game, bdir, st, &rl)?;
    write_file(&game.join(&rl), REALMLIST)?;
    st.wrote.insert(rl.clone(), sha(REALMLIST));
    out.lines.push("realmlist.wtf: this PC (the old one is kept and comes back with Sign-in off)".into());
    // 2. Config.wtf: the account name and the realm (and realmList when the file has that line)
    let cfg = rel_ci(game, Some("WTF"), "Config.wtf");
    keep_original(game, bdir, st, &cfg)?;
    let now = fs::read(game.join(&cfg)).map(|b| latin1(&b)).unwrap_or_default();
    let mut keys = vec![("accountName", ACCOUNT), ("realmName", REALM)];
    if wtf_value(&now, "realmList").is_some() {
        keys.push(("realmList", "127.0.0.1"));
    }
    let mut text = now.clone();
    for (k, v) in &keys {
        st.cfg_old.entry(k.to_string()).or_insert_with(|| wtf_value(&now, k));
        text = set_wtf_line(&text, k, v);
    }
    let bytes = unlatin1(&text);
    write_file(&game.join(&cfg), &bytes)?;
    st.wrote.insert(cfg, sha(&bytes));
    out.lines.push(format!("Config.wtf: the account name {ACCOUNT} and the realm \"{REALM}\" filled in"));
    // 3. the add-on (addon mode)
    if mode == Mode::Addon {
        match write_addon(game, st, session) {
            Ok(line) => {
                out.addon = true;
                out.lines.push(line);
                st.why.clear();
            }
            Err(e) => {
                out.why = e.clone();
                st.why = e;
                out.lines.push(format!("The sign-in add-on isn't used this time ({}): type the password once.", out.why));
            }
        }
    } else {
        // prefill: an archive of ours from an addon start goes
        for (rel, _) in std::mem::take(&mut st.added) {
            let p = game.join(&rel);
            if p.exists() && is_ours(&p) {
                fs::remove_file(&p).map_err(|e| format!("Couldn't remove {rel} (is the game open?): {e}"))?;
            }
        }
        st.why.clear();
    }
    Ok(out)
}

fn write_addon(game: &Path, st: &mut SignState, session: Option<&Session>) -> Result<String, String> {
    let d = data_dir(game).ok_or("this game folder has no Data folder")?;
    let slot = crate::clients::find_ci(&d, SLOT);
    let dname = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Data".into());
    let rel = format!("{dname}/{}", slot.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| SLOT.into()));
    if let Some(p) = &slot {
        if !st.added.contains_key(&rel) && !is_ours(p) {
            return Err(format!("{SLOT} in your Data folder is another mod's, so it is left as it is"));
        }
    }
    let (toc, src) = player_toc(game, SLOT)?;
    let b = archive(&toc_with_ours(&toc), &session_lua(session)?)?;
    write_file(&game.join(&rel), &b)?;
    st.added.insert(rel.clone(), sha(&b));
    Ok(format!("The sign-in add-on: {} (after the login screen's file list from your {src})", rel.replace('/', "\\")))
}

/// The password out of our archive (the add-on stays, with no session): after the game let go of it, and on stop.
/// Ok(true) when one was taken out. Err while the game holds the archive.
pub fn scrub_session(st: &mut SignState) -> Result<bool, String> {
    if !st.on {
        return Ok(false);
    }
    let game = PathBuf::from(&st.game);
    let mut any = false;
    for (rel, want) in st.added.clone() {
        let p = game.join(&rel);
        let Ok(b) = fs::read(&p) else { continue };
        if sha(&b) != want {
            continue;
        }
        if mpq::read(&b, SESSION).is_some_and(|s| s == NO_SESSION.as_bytes()) {
            continue;
        }
        let toc = mpq::read(&b, TOC).map(|t| latin1(&t)).ok_or("our archive has no file list")?;
        let nb = archive(&toc, NO_SESSION)?;
        write_file(&p, &nb)?;
        st.added.insert(rel, sha(&nb));
        any = true;
    }
    Ok(any)
}

/// Whether a sign-in is in our archive now.
pub fn has_session(st: &SignState) -> bool {
    let game = PathBuf::from(&st.game);
    st.added.keys().any(|rel| fs::read(game.join(rel)).ok().and_then(|b| mpq::read(&b, SESSION)).is_some_and(|s| s != NO_SESSION.as_bytes()))
}

/// Sign-in off: our archive removed (only when it is ours), every changed file put back byte for byte with its old
/// modified time (or removed when it wasn't there). Config.wtf that the game rewrote since keeps the player's new
/// settings, with only our lines put back as they were. The lines say what was done; the state is cleared when all
/// went well.
pub fn restore(bdir: &Path, st: &mut SignState) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    if !st.on {
        lines.push("Nothing of Leyline Solo's sign-in is in a game folder.".into());
        return Ok(lines);
    }
    let game = PathBuf::from(&st.game);
    let mut problems = Vec::new();
    for (rel, want) in &st.added {
        let p = game.join(rel);
        if !p.exists() {
            continue;
        }
        let same = fs::read(&p).map(|b| sha(&b) == *want).unwrap_or(false);
        if same || is_ours(&p) {
            match fs::remove_file(&p) {
                Ok(()) => lines.push(format!("Removed {} (ours).", rel.replace('/', "\\"))),
                Err(e) => problems.push(format!("Couldn't remove {} (is the game open? close it first): {e}", rel.replace('/', "\\"))),
            }
        } else {
            lines.push(format!("{} changed since Leyline wrote it, so it was left as it is.", rel.replace('/', "\\")));
        }
    }
    for (rel, name) in &st.changed {
        let p = game.join(rel);
        let rewritten = st.wrote.get(rel).is_some_and(|w| fs::read(&p).map(|b| sha(&b) != *w).unwrap_or(false));
        let r = if rel.to_ascii_lowercase().ends_with("config.wtf") && rewritten && p.is_file() {
            // the game wrote its settings since: keep them, put only our lines back as they were
            let mut text = fs::read(&p).map(|b| latin1(&b)).unwrap_or_default();
            for (k, old) in &st.cfg_old {
                text = match old {
                    Some(v) => set_wtf_line(&text, k, v),
                    None => remove_wtf_line(&text, k),
                };
            }
            write_file(&p, &unlatin1(&text)).map(|_| "Config.wtf: your game's settings kept, the account name and realm put back as they were.".to_string())
        } else if name.is_empty() {
            if p.exists() {
                fs::remove_file(&p).map(|_| format!("Removed {} (it wasn't there before).", rel.replace('/', "\\"))).map_err(|e| e.to_string())
            } else {
                Ok(String::new())
            }
        } else {
            let b = bdir.join(name);
            fs::read(&b).map_err(|e| format!("its backup is missing ({e})")).and_then(|bytes| {
                write_file(&p, &bytes)?;
                if let Some(t) = mtime(&b) {
                    set_mtime(&p, t);
                }
                Ok(format!("Put back {} as it was.", rel.replace('/', "\\")))
            })
        };
        match r {
            Ok(l) if !l.is_empty() => lines.push(l),
            Ok(_) => {}
            Err(e) => problems.push(format!("Couldn't put back {}: {e}", rel.replace('/', "\\"))),
        }
    }
    if !problems.is_empty() {
        return Err(problems.join(" "));
    }
    for name in st.changed.values().filter(|n| !n.is_empty()) {
        let _ = fs::remove_file(bdir.join(name));
    }
    *st = SignState::default();
    lines.push("Your game folder is as it was.".into());
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOC_TEXT: &str = "## Interface: 11200\r\nGlueLocalization.lua\r\nGlueParent.xml\r\nAccountLogin.xml\r\nCharacterSelect.xml\r\n";

    fn game(toc_in_patch: bool) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let g = d.path();
        fs::create_dir_all(g.join("Data")).unwrap();
        fs::create_dir_all(g.join("WTF")).unwrap();
        fs::write(g.join("realmlist.wtf"), b"set realmlist logon.example.org\r\n").unwrap();
        fs::write(g.join("WTF/Config.wtf"), b"SET gxResolution \"1024x768\"\r\nSET accountName \"OLDNAME\"\r\n").unwrap();
        fs::write(g.join("Data/interface.MPQ"), mpq::build(&[(TOC.into(), b"## Interface: 11200\r\nGlueParent.xml\r\n".to_vec())]).unwrap()).unwrap();
        if toc_in_patch {
            fs::write(g.join("Data/patch.MPQ"), mpq::build(&[(TOC.into(), TOC_TEXT.as_bytes().to_vec())]).unwrap()).unwrap();
        }
        fs::write(g.join("Data/dbc.MPQ"), mpq::build(&[("x.txt".into(), b"placeholder".to_vec())]).unwrap()).unwrap();
        d
    }

    fn snapshot(dir: &Path) -> BTreeMap<String, (Vec<u8>, u64)> {
        let mut out = BTreeMap::new();
        fn walk(d: &Path, root: &Path, out: &mut BTreeMap<String, (Vec<u8>, u64)>) {
            for e in fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    let t = fs::metadata(&p).unwrap().modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                    out.insert(p.strip_prefix(root).unwrap().display().to_string(), (fs::read(&p).unwrap(), t));
                }
            }
        }
        walk(dir, dir, &mut out);
        out
    }

    fn age(dir: &Path) {
        // every file a day old, so a restored time is told from a new one
        let old = SystemTime::now() - std::time::Duration::from_secs(86_400);
        for (rel, _) in snapshot(dir) {
            set_mtime(&dir.join(rel), old);
        }
    }

    fn sess() -> Session {
        Session { user: "SOLO".into(), pass: "ABCDEFGH23456789".into(), realm: REALM.into(), pick: Some("Wren".into()), make: None }
    }

    #[test]
    fn modes_read_as_the_setting_says() {
        assert_eq!(Mode::from_word("addon"), Mode::Addon);
        assert_eq!(Mode::from_word(""), Mode::Prefill, "the default");
        assert_eq!(Mode::from_word("other"), Mode::Prefill);
        assert!(Mode::parse("other").is_err());
        assert_eq!(Mode::parse(" ADDON ").unwrap().word(), "addon");
        assert_eq!((Mode::parse("simple").unwrap(), Mode::parse("quick").unwrap()), (Mode::Prefill, Mode::Addon), "the page's words");
        assert_eq!((Mode::Prefill.ui_word(), Mode::from_word("quick").ui_word()), ("simple", "quick"));
    }

    #[test]
    fn config_lines_are_set_kept_and_removed() {
        let t = "SET gxResolution \"1024x768\"\r\nSET accountName \"OLDNAME\"\r\n";
        let s = set_wtf_line(t, "accountName", "SOLO");
        assert_eq!(s, "SET gxResolution \"1024x768\"\r\nSET accountName \"SOLO\"\r\n");
        assert_eq!(set_wtf_line(&s, "realmName", "Leyline Solo"), "SET gxResolution \"1024x768\"\r\nSET accountName \"SOLO\"\r\nSET realmName \"Leyline Solo\"\r\n");
        assert_eq!(set_wtf_line("", "accountName", "SOLO"), "SET accountName \"SOLO\"\r\n", "a new file gets the game's CRLF");
        assert_eq!(set_wtf_line("SET a \"1\"\n", "b", "2"), "SET a \"1\"\nSET b \"2\"\n", "LF kept");
        assert_eq!(wtf_value(t, "accountname").as_deref(), Some("OLDNAME"), "any case");
        assert_eq!(wtf_value(t, "realmName"), None);
        assert_eq!(remove_wtf_line(&s, "accountName"), "SET gxResolution \"1024x768\"\r\n");
        assert!(!is_key("SET accountNameX \"a\"", "accountName"));
    }

    #[test]
    fn the_file_list_gets_our_line_once() {
        assert_eq!(toc_with_ours(TOC_TEXT), format!("{TOC_TEXT}LeylineSolo.xml\r\n"));
        assert_eq!(toc_with_ours(&toc_with_ours(TOC_TEXT)), toc_with_ours(TOC_TEXT));
        assert_eq!(toc_with_ours("A.xml\nB.xml\n\n"), "A.xml\r\nB.xml\r\nLeylineSolo.xml\r\n");
        let names: Vec<String> = ["dbc.MPQ", "interface.MPQ", "patch.MPQ", "patch-2.MPQ", "patch-Z.MPQ", "patch-A.MPQ", "patch-enUS.MPQ"].iter().map(|s| s.to_string()).collect();
        assert_eq!(slot_order(&names, SLOT), ["patch-A.MPQ", "patch-2.MPQ", "patch.MPQ", "interface.MPQ"], "the last read first; ours never");
    }

    #[test]
    fn the_session_is_escaped_bytes_in_the_shapes_the_game_takes() {
        let s = session_lua(Some(&sess())).unwrap();
        assert!(!s.contains("\"SOLO\"") && !s.contains("ABCDEFGH") && !s.contains("Wren") && !s.contains("Leyline Solo\""), "{s}");
        assert!(s.contains(", pick = \"\\087\\114"));
        assert_eq!(session_lua(None).unwrap(), NO_SESSION);
        let m = session_lua(Some(&Session { pick: None, make: Some(NewChar { name: "Tamsin".into(), race: "NightElf".into(), class: "DRUID".into(), female: true }), ..sess() })).unwrap();
        assert!(m.contains(", make = { name = ") && m.contains("female = true }") && !m.contains("NightElf"), "{m}");
        for bad in [Session { pass: "lower1".into(), ..sess() }, Session { pass: "".into(), ..sess() }, Session { user: "SO LO".into(), ..sess() },
            Session { pass: "A".repeat(17), ..sess() }, Session { realm: "a\"b".into(), ..sess() }, Session { pick: Some("wren".into()), ..sess() },
            Session { make: Some(NewChar { name: "Tamsin".into(), race: "Night Elf".into(), class: "DRUID".into(), female: false }), ..sess() }] {
            assert!(session_lua(Some(&bad)).is_err(), "{bad:?}");
        }
        assert!(!format!("{:?}", sess()).contains("ABCDEFGH"), "never in a debug line");
        // in a real Lua (CI has 5.1) it gives back exactly the values
        let lua = ["lua5.1", "lua", "texlua"].iter().find(|l| std::process::Command::new(l).arg("-v").output().is_ok());
        if let Some(lua) = lua {
            let code = format!("{s}\nio.write(LEYLINE_SOLO_SESSION.user, '|', LEYLINE_SOLO_SESSION.pass, '|', LEYLINE_SOLO_SESSION.realm, '|', LEYLINE_SOLO_SESSION.pick)");
            let f = tempfile::NamedTempFile::new().unwrap();
            fs::write(f.path(), code).unwrap();
            let out = std::process::Command::new(lua).arg(f.path()).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), "SOLO|ABCDEFGH23456789|Leyline Solo|Wren");
        }
    }

    #[test]
    fn prefill_on_then_off_puts_every_byte_and_time_back() {
        let d = game(true);
        let b = tempfile::tempdir().unwrap();
        age(d.path());
        let before = snapshot(d.path());
        let mut st = SignState::default();
        let r = install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        assert!(!r.addon && r.why.is_empty());
        assert_eq!(fs::read(d.path().join("realmlist.wtf")).unwrap(), REALMLIST);
        let cfg = String::from_utf8(fs::read(d.path().join("WTF/Config.wtf")).unwrap()).unwrap();
        assert_eq!(cfg, "SET gxResolution \"1024x768\"\r\nSET accountName \"SOLO\"\r\nSET realmName \"Leyline Solo\"\r\n");
        assert!(!d.path().join("Data").join(SLOT).exists(), "prefill: no archive");
        // twice: the first backup (the player's own) is kept
        install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        let lines = restore(b.path(), &mut st).unwrap();
        assert!(lines.last().unwrap().contains("as it was"));
        assert_eq!(snapshot(d.path()), before, "every byte and modified time back");
        assert_eq!(st, SignState::default());
        assert_eq!(fs::read_dir(b.path()).unwrap().count(), 0, "the backups go after");
    }

    #[test]
    fn files_that_were_not_there_are_removed_again() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data")).unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut st = SignState::default();
        install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        assert!(d.path().join("realmlist.wtf").is_file() && d.path().join("WTF/Config.wtf").is_file());
        restore(b.path(), &mut st).unwrap();
        assert!(!d.path().join("realmlist.wtf").exists() && !d.path().join("WTF/Config.wtf").exists());
    }

    #[test]
    fn addon_writes_our_archive_with_the_players_list_and_takes_the_password_out() {
        let d = game(true);
        let b = tempfile::tempdir().unwrap();
        age(d.path());
        let before = snapshot(d.path());
        let mut st = SignState::default();
        let r = install(d.path(), b.path(), &mut st, Mode::Addon, Some(&sess())).unwrap();
        assert!(r.addon, "{r:?}");
        let a = fs::read(d.path().join("Data").join(SLOT)).unwrap();
        assert_eq!(String::from_utf8(mpq::read(&a, TOC).unwrap()).unwrap(), format!("{TOC_TEXT}LeylineSolo.xml\r\n"), "patch.MPQ's list (read last) plus ours");
        assert_eq!(mpq::read(&a, OUR_LUA).unwrap(), GLUE_LUA);
        assert_eq!(mpq::read(&a, OUR_XML).unwrap(), GLUE_XML);
        assert!(has_session(&st));
        assert!(!String::from_utf8_lossy(&a).contains("ABCDEFGH23456789"), "never plain in the archive");
        assert!(scrub_session(&mut st).unwrap());
        assert!(!has_session(&st));
        assert!(!scrub_session(&mut st).unwrap(), "nothing left to take out");
        let a = fs::read(d.path().join("Data").join(SLOT)).unwrap();
        assert_eq!(mpq::read(&a, SESSION).unwrap(), NO_SESSION.as_bytes());
        assert!(mpq::read(&a, TOC).unwrap().ends_with(b"LeylineSolo.xml\r\n"), "the list stays");
        restore(b.path(), &mut st).unwrap();
        assert_eq!(snapshot(d.path()), before, "our archive gone, everything else back");
    }

    #[test]
    fn addon_falls_back_to_prefill_when_the_list_cant_be_read_or_the_slot_is_taken() {
        // no archive holds the list
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data")).unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut st = SignState::default();
        let r = install(d.path(), b.path(), &mut st, Mode::Addon, Some(&sess())).unwrap();
        assert!(!r.addon && r.why.contains("file list"), "{r:?}");
        assert!(!d.path().join("Data").join(SLOT).exists());
        assert_eq!(fs::read(d.path().join("realmlist.wtf")).unwrap(), REALMLIST, "the prefill is still done");
        restore(b.path(), &mut st).unwrap();
        // someone else's archive in the slot: never touched
        let d = game(true);
        let theirs = mpq::build(&[("Interface\\Other\\x.lua".into(), b"theirs".to_vec())]).unwrap();
        fs::write(d.path().join("Data").join(SLOT), &theirs).unwrap();
        let mut st = SignState::default();
        let r = install(d.path(), b.path(), &mut st, Mode::Addon, Some(&sess())).unwrap();
        assert!(!r.addon && r.why.contains("another mod"), "{r:?}");
        restore(b.path(), &mut st).unwrap();
        assert_eq!(fs::read(d.path().join("Data").join(SLOT)).unwrap(), theirs, "left as it is");
    }

    #[test]
    fn switching_to_prefill_takes_our_archive_out() {
        let d = game(true);
        let b = tempfile::tempdir().unwrap();
        let mut st = SignState::default();
        assert!(install(d.path(), b.path(), &mut st, Mode::Addon, Some(&sess())).unwrap().addon);
        install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        assert!(!d.path().join("Data").join(SLOT).exists());
        assert!(st.added.is_empty());
    }

    #[test]
    fn a_config_the_game_rewrote_keeps_its_settings_and_gets_our_lines_back() {
        let d = game(false);
        let b = tempfile::tempdir().unwrap();
        let mut st = SignState::default();
        install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        // the game closes and writes its settings, with a new one
        fs::write(d.path().join("WTF/Config.wtf"), "SET gxResolution \"1920x1080\"\r\nSET accountName \"SOLO\"\r\nSET realmName \"Leyline Solo\"\r\nSET gamma \"1.2\"\r\n").unwrap();
        let lines = restore(b.path(), &mut st).unwrap();
        assert!(lines.iter().any(|l| l.contains("settings kept")), "{lines:?}");
        assert_eq!(String::from_utf8(fs::read(d.path().join("WTF/Config.wtf")).unwrap()).unwrap(),
            "SET gxResolution \"1920x1080\"\r\nSET accountName \"OLDNAME\"\r\nSET gamma \"1.2\"\r\n", "the old name back, our realm line gone, theirs kept");
        assert_eq!(fs::read(d.path().join("realmlist.wtf")).unwrap(), b"set realmlist logon.example.org\r\n");
    }

    #[test]
    fn another_folder_is_refused_while_one_has_it_on() {
        let (d, e) = (game(false), game(false));
        let b = tempfile::tempdir().unwrap();
        let mut st = SignState::default();
        install(d.path(), b.path(), &mut st, Mode::Prefill, None).unwrap();
        assert!(install(e.path(), b.path(), &mut st, Mode::Prefill, None).unwrap_err().contains("another game folder"));
        assert_eq!(restore(b.path(), &mut SignState::default()).unwrap(), ["Nothing of Leyline Solo's sign-in is in a game folder."]);
    }

    /// The add-on's own tests (glue/test_solo_glue.lua: a stand-in first-era login screen) in a real Lua, with a
    /// Quick start session this module wrote, when one is installed here (CI installs lua5.1).
    #[test]
    fn the_glue_signs_in_picks_makes_and_steps_aside_on_anything_unexpected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("glue");
        let Some(lua) = ["lua5.1", "lua", "texlua"].iter().find(|l| std::process::Command::new(l).arg("-v").output().is_ok()) else { return };
        let s = session_lua(Some(&Session { pick: None, make: Some(NewChar { name: "Tamsin".into(), race: "NightElf".into(), class: "DRUID".into(), female: true }), ..sess() })).unwrap();
        let f = tempfile::NamedTempFile::new().unwrap();
        fs::write(f.path(), s).unwrap();
        let out = std::process::Command::new(lua).arg(root.join("test_solo_glue.lua")).arg(root.join("LeylineSolo.lua")).arg(f.path()).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success() && text.contains("all passed"), "{text}{}", String::from_utf8_lossy(&out.stderr));
    }

    /// The add-on is Lua 5.0 (the first-era login screen's), in our own words, and the same file Phase 0's tool packs.
    #[test]
    fn the_glue_is_lua_50_our_words_and_phase_0s_copy() {
        let lua = String::from_utf8(GLUE_LUA.to_vec()).unwrap();
        let mut code = String::new();
        for line in lua.lines() {
            let l = line.split("--").next().unwrap_or("");
            // strings out
            let mut out = String::new();
            let mut in_str = false;
            let mut esc = false;
            for c in l.chars() {
                if in_str {
                    if esc {
                        esc = false;
                    } else if c == '\\' {
                        esc = true;
                    } else if c == '"' {
                        in_str = false;
                    }
                } else if c == '"' {
                    in_str = true;
                } else {
                    out.push(c);
                }
            }
            code.push_str(&out);
            code.push('\n');
        }
        for bad in ["#", "%", "string.match", ":match(", "gmatch", "select(", "...", "goto ", "//"] {
            assert!(!code.contains(bad), "Lua 5.0 has no {bad}");
        }
        for f in [GLUE_LUA, GLUE_XML] {
            let t = String::from_utf8_lossy(f).replace("http://www.blizzard.com/wow/ui/", "");
            for w in ["World of Warcraft", "Warcraft", "Lich King", "Blizzard"] {
                assert!(!t.contains(w), "{w}");
            }
            assert!(!t.split(|c: char| !c.is_ascii_alphanumeric()).any(|x| x == "WoW"));
        }
        let x = String::from_utf8_lossy(GLUE_XML);
        assert!(x.find("Script file=\"LeylineSoloSession.lua\"").unwrap() < x.find("Script file=\"LeylineSolo.lua\"").unwrap());
        let phase0 = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/solo/glue");
        if phase0.is_dir() {
            assert!(fs::read(phase0.join("LeylineSolo.lua")).unwrap() == GLUE_LUA, "tools/solo/glue/LeylineSolo.lua is this file");
            assert!(fs::read(phase0.join("LeylineSolo.xml")).unwrap() == GLUE_XML, "tools/solo/glue/LeylineSolo.xml is this file");
        }
    }
}
