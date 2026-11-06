//! Loadouts (owner 3 Nov: "style / loadout for clients ... load various versions of their games, templates and other
//! people's realms without losing game file access and ease"): a named set of mods, addons and looks, with a graphics
//! preset, for one game folder. "Clean", "Raider", "HD looks", "My friend's realm".
//!
//! What switching does, and what it never does:
//! * Only what Leyline installed moves (the mod manager's own list, `<client>/Leyline/mods.json`): a mod the new loadout
//!   does not use is turned off (`mods::set_enabled`: its files wait in `<client>/Leyline/disabled/`, where the game
//!   does not look), one it uses is turned back on. The game's own files are never deleted or changed, and addons the
//!   player put in by hand stay exactly as they are.
//! * Before a switch, the loadout being left gets a copy of the game's `WTF` folder (Config.wtf, key bindings, addon
//!   settings) in `<client>/Leyline/loadouts/<id>/WTF/`. Nothing is copied back by itself: it is a backup.
//! * The loadout's graphics preset (gfx.rs, checked values only) is written; `Clean` puts the graphics settings back
//!   as they were before Leyline first changed them (gfx.rs keeps that copy).
//! * `Clean` is always there: every Leyline mod off, the game as installed. One click back.
//! * While a loadout is on, it follows the player: what they turn on or install is in it the next time they leave it.
//!
//! Sharing is a short code of catalog ids only (`share_code`, `parse_code`), never files or addresses: a code names
//! what to install, and the launcher only ever installs ids it finds in its own reviewed catalog. Templates (the UI's
//! list, from the website's `stream/loadouts.json`) and a realm's suggested loadout (realms.rs `RealmLoadout`) come in
//! the same shape (`Shared`).

use crate::mods::{self, Manifest};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// In `<client>/Leyline/`.
pub const FILE: &str = "loadouts.json";
/// In `<client>/Leyline/`: the per-loadout WTF copies.
pub const BACKUP_DIR: &str = "loadouts";
pub const SCHEMA: u32 = 1;
/// The loadout every game folder has: every Leyline mod off.
pub const CLEAN: &str = "clean";
pub const MAX_LOADOUTS: usize = 24;
pub const MAX_ITEMS: usize = 64;
pub const MAX_NAME: usize = 40;
/// A share code starts with this.
pub const CODE_PREFIX: &str = "leyline-loadout:1;";
pub const MAX_CODE: usize = 4000;
/// The WTF copy: at most this many files and bytes (a WTF folder is normally a few MB).
const WTF_MAX_FILES: usize = 4000;
const WTF_MAX_BYTES: u64 = 64 * 1024 * 1024;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// One loadout.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loadout {
    pub id: String,
    pub name: String,
    /// Mod manager keys (`mod:<id>`, `up:<slug>`, `look:<slug>`, `leyline:addons`, or a mod from the player's PC:
    /// `local:<addon>`, `patch:<file>`).
    pub items: Vec<String>,
    /// A gfx.rs preset id, or "" (graphics as they are).
    pub gfx: String,
    /// Where it came from: "" (made here), `template:<id>`, `realm:<slug>`, `code`.
    pub from: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Book {
    pub schema: u32,
    /// The loadout that is on: `clean`, a loadout id, or "" (none chosen yet: the game as the player set it up).
    pub active: String,
    /// The one before it (for "Back to ..." after a realm's loadout).
    pub previous: String,
    pub loadouts: Vec<Loadout>,
}

impl Book {
    pub fn get(&self, id: &str) -> Option<&Loadout> {
        self.loadouts.iter().find(|l| l.id == id)
    }
}

/// A loadout as it travels: a template, a realm's suggestion or a pasted code. Ids only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shared {
    pub name: String,
    pub items: Vec<String>,
    pub gfx: String,
}

// ------------------------------------------------------------------------------------------------ checks

/// A name a player typed: control characters out, trimmed, at most `MAX_NAME` characters. None when nothing is left.
pub fn clean_name(s: &str) -> Option<String> {
    let t: String = s.chars().filter(|c| !c.is_control()).collect();
    let t: String = t.trim().chars().take(MAX_NAME).collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// An id a code, a template or a realm may name: a catalog id, or the Leyline addons.
pub fn shareable(id: &str) -> bool {
    id == crate::addons::KEY || crate::catalog::id_ok(id)
}

/// A key the player's own loadout may hold (a shareable id, or a mod from their PC as the mod manager names it).
pub fn item_ok(key: &str) -> bool {
    shareable(key)
        || ((key.starts_with("local:") || key.starts_with("patch:"))
            && key.len() <= 128
            && !key.chars().any(|c| c.is_control()))
}

/// A gfx.rs preset id, or "".
pub fn gfx_ok(g: &str) -> bool {
    g.is_empty() || crate::gfx::PRESETS.iter().any(|(id, _, _)| *id == g)
}

/// A shared loadout made safe: a name, shareable ids (each once, at most `MAX_ITEMS`), a known preset.
pub fn clean_shared(s: &Shared) -> Result<Shared, String> {
    let name = clean_name(&s.name).ok_or("A loadout needs a name.")?;
    let mut items: Vec<String> = Vec::new();
    for i in &s.items {
        if shareable(i) && !items.contains(i) {
            items.push(i.clone());
        }
        if items.len() >= MAX_ITEMS {
            break;
        }
    }
    let gfx = if gfx_ok(&s.gfx) {
        s.gfx.clone()
    } else {
        String::new()
    };
    Ok(Shared { name, items, gfx })
}

// ------------------------------------------------------------------------------------------------ share codes

fn pct(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unpct(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The share code of a loadout: `leyline-loadout:1;n=<name>;g=<preset>;i=<id>,<id>`. Only shareable ids go in (a mod
/// from the player's own PC has nothing anyone else could install).
pub fn share_code(l: &Loadout) -> String {
    let ids: Vec<&str> = l
        .items
        .iter()
        .map(String::as_str)
        .filter(|i| shareable(i))
        .collect();
    format!(
        "{CODE_PREFIX}n={};g={};i={}",
        pct(&l.name),
        pct(&l.gfx),
        ids.join(",")
    )
}

/// A pasted code, checked. Unknown parts are ignored; ids that are not catalog ids are dropped.
pub fn parse_code(code: &str) -> Result<Shared, String> {
    let c = code.trim();
    if c.len() > MAX_CODE {
        return Err("That code is too long to be a loadout.".into());
    }
    let rest = c
        .strip_prefix(CODE_PREFIX)
        .ok_or("That isn't a Leyline loadout code.")?;
    let mut s = Shared::default();
    for part in rest.split(';') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        match k {
            "n" => s.name = unpct(v).ok_or("That loadout code is damaged.")?,
            "g" => s.gfx = unpct(v).unwrap_or_default(),
            "i" => {
                s.items = v
                    .split(',')
                    .filter(|x| !x.is_empty())
                    .map(String::from)
                    .collect()
            }
            _ => {}
        }
    }
    if s.name.trim().is_empty() {
        s.name = "Shared loadout".into();
    }
    clean_shared(&s)
}

// ------------------------------------------------------------------------------------------------ the plan

/// What a switch does to the mods Leyline installed.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Plan {
    /// Turned on (installed, off now, in the loadout).
    pub enable: Vec<String>,
    /// Turned off (installed, on now, not in the loadout).
    pub disable: Vec<String>,
    /// In the loadout, not installed in this game folder: the UI offers to install them (catalog ids only).
    pub missing: Vec<String>,
}

pub fn plan(man: &Manifest, items: &[String]) -> Plan {
    let mut p = Plan::default();
    for m in &man.mods {
        let want = items.iter().any(|i| *i == m.key);
        if want && !m.enabled {
            p.enable.push(m.key.clone());
        } else if !want && m.enabled {
            p.disable.push(m.key.clone());
        }
    }
    for i in items {
        if man.get(i).is_none() && shareable(i) && !p.missing.contains(i) {
            p.missing.push(i.clone());
        }
    }
    p
}

/// The keys of the mods that are on now.
pub fn enabled_now(man: &Manifest) -> Vec<String> {
    man.mods
        .iter()
        .filter(|m| m.enabled)
        .map(|m| m.key.clone())
        .collect()
}

/// A new loadout id from a name: lower-case letters, digits and dashes, not `clean`, not taken.
pub fn new_id(book: &Book, name: &str) -> String {
    let mut base: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(24);
    let base = base.trim_end_matches('-').to_string();
    let base = if base.is_empty() || base == CLEAN {
        "loadout".to_string()
    } else {
        base
    };
    let mut id = base.clone();
    let mut n = 2;
    while id == CLEAN || book.get(&id).is_some() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

// ------------------------------------------------------------------------------------------------ the file

pub fn path(client: &Path) -> PathBuf {
    client.join(mods::DIR).join(FILE)
}

pub fn load(client: &Path) -> Book {
    let mut b: Book = fs::read(path(client))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    b.loadouts.retain(|l| !l.id.is_empty() && l.id != CLEAN);
    b.loadouts.truncate(MAX_LOADOUTS);
    for l in &mut b.loadouts {
        l.items.retain(|i| item_ok(i));
        if !gfx_ok(&l.gfx) {
            l.gfx.clear();
        }
    }
    if b.active != CLEAN && !b.active.is_empty() && b.get(&b.active.clone()).is_none() {
        b.active.clear();
    }
    b
}

pub fn save(client: &Path, b: &Book) -> Result<(), String> {
    let mut b = b.clone();
    b.schema = SCHEMA;
    let mut bytes = serde_json::to_vec_pretty(&b).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let p = path(client);
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("Couldn't save the loadouts: {e}"))?;
    }
    crate::settings::write_atomic(&p, &bytes)
        .map_err(|e| format!("Couldn't save the loadouts: {e}"))
}

/// The loadout that is on follows what the player did while it was on (what they turned on, installed or removed).
fn sync_active(book: &mut Book, man: &Manifest) {
    let on = enabled_now(man);
    let active = book.active.clone();
    if let Some(l) = book.loadouts.iter_mut().find(|l| l.id == active) {
        // what is in the loadout but not installed stays (it may be installed later); what is installed follows
        let mut items: Vec<String> = l
            .items
            .iter()
            .filter(|i| man.get(i).map_or(true, |m| m.enabled))
            .cloned()
            .collect();
        for k in on {
            if !items.contains(&k) {
                items.push(k);
            }
        }
        items.truncate(MAX_ITEMS);
        l.items = items;
    }
}

/// Add a loadout. `items` None: what is on now.
pub fn add(
    client: &Path,
    name: &str,
    items: Option<Vec<String>>,
    gfx: &str,
    from: &str,
) -> Result<Loadout, String> {
    let mut book = load(client);
    if book.loadouts.len() >= MAX_LOADOUTS {
        return Err(format!(
            "A game folder keeps at most {MAX_LOADOUTS} loadouts. Delete one first."
        ));
    }
    let name = clean_name(name).ok_or("A loadout needs a name.")?;
    let man = mods::load(client);
    sync_active(&mut book, &man);
    let wanted = items.unwrap_or_else(|| enabled_now(&man));
    let mut items: Vec<String> = Vec::new();
    for i in wanted {
        if item_ok(&i) && !items.contains(&i) {
            items.push(i);
        }
    }
    items.truncate(MAX_ITEMS);
    let l = Loadout {
        id: new_id(&book, &name),
        name,
        items,
        gfx: if gfx_ok(gfx) {
            gfx.to_string()
        } else {
            String::new()
        },
        from: from.chars().filter(|c| !c.is_control()).take(80).collect(),
        created_at: now(),
    };
    book.loadouts.push(l.clone());
    save(client, &book)?;
    Ok(l)
}

/// Add a shared loadout (a template, a realm's, a code), checked first.
pub fn add_shared(client: &Path, s: &Shared, from: &str) -> Result<Loadout, String> {
    let s = clean_shared(s)?;
    add(client, &s.name, Some(s.items), &s.gfx, from)
}

pub fn copy(client: &Path, id: &str, name: &str) -> Result<Loadout, String> {
    let mut book = load(client);
    sync_active(&mut book, &mods::load(client));
    save(client, &book)?;
    let l = book
        .get(id)
        .cloned()
        .ok_or("That loadout isn't there any more.")?;
    add(client, name, Some(l.items), &l.gfx, &l.from)
}

pub fn rename(client: &Path, id: &str, name: &str) -> Result<Loadout, String> {
    let mut book = load(client);
    let name = clean_name(name).ok_or("A loadout needs a name.")?;
    let l = book
        .loadouts
        .iter_mut()
        .find(|l| l.id == id)
        .ok_or("That loadout isn't there any more.")?;
    l.name = name;
    let out = l.clone();
    save(client, &book)?;
    Ok(out)
}

pub fn set_gfx(client: &Path, id: &str, gfx: &str) -> Result<Loadout, String> {
    if !gfx_ok(gfx) {
        return Err("No such graphics preset.".into());
    }
    let mut book = load(client);
    let l = book
        .loadouts
        .iter_mut()
        .find(|l| l.id == id)
        .ok_or("That loadout isn't there any more.")?;
    l.gfx = gfx.to_string();
    let out = l.clone();
    save(client, &book)?;
    Ok(out)
}

/// Delete a loadout (never the one that is on). Its WTF copy goes with it; no mod is touched.
pub fn delete(client: &Path, id: &str) -> Result<(), String> {
    let mut book = load(client);
    if book.active == id {
        return Err("That loadout is on. Switch to another one first.".into());
    }
    let before = book.loadouts.len();
    book.loadouts.retain(|l| l.id != id);
    if book.loadouts.len() == before {
        return Err("That loadout isn't there any more.".into());
    }
    if book.previous == id {
        book.previous.clear();
    }
    let _ = fs::remove_dir_all(client.join(mods::DIR).join(BACKUP_DIR).join(id));
    save(client, &book)
}

// ------------------------------------------------------------------------------------------------ the WTF copy

/// Copy the game's WTF folder into `dest` (replacing an older copy). Links are skipped; stops at the caps. Returns
/// the number of files copied.
pub fn backup_wtf(client: &Path, dest: &Path) -> Result<usize, String> {
    let Some(wtf) = crate::clients::find_ci(client, "WTF") else {
        return Ok(0);
    };
    let _ = fs::remove_dir_all(dest);
    let mut stack = vec![wtf.clone()];
    let (mut files, mut bytes) = (0usize, 0u64);
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(md) = fs::symlink_metadata(&p) else {
                continue;
            };
            if md.file_type().is_symlink() {
                continue;
            }
            if md.is_dir() {
                stack.push(p);
                continue;
            }
            if files >= WTF_MAX_FILES || bytes + md.len() > WTF_MAX_BYTES {
                return Ok(files);
            }
            let rel = p.strip_prefix(&wtf).map_err(|e| e.to_string())?;
            let to = dest.join(rel);
            if let Some(d) = to.parent() {
                fs::create_dir_all(d)
                    .map_err(|e| format!("Couldn't copy the game's settings: {e}"))?;
            }
            fs::copy(&p, &to).map_err(|e| format!("Couldn't copy the game's settings: {e}"))?;
            files += 1;
            bytes += md.len();
        }
    }
    Ok(files)
}

// ------------------------------------------------------------------------------------------------ switching

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Switched {
    pub active: String,
    pub name: String,
    pub previous: String,
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
    /// In the loadout, not in this game folder yet (the UI offers to install them).
    pub missing: Vec<String>,
    /// Mods that could not be moved (their place is taken, say), with why.
    pub problems: Vec<String>,
    /// Where the WTF copy of the loadout that was on went (client-relative), "" when there was none.
    pub backup: String,
    /// The graphics: "preset", "original", "" (left as they are), or why not.
    pub gfx: String,
}

/// Switch the game folder to loadout `id` (`clean` for every Leyline mod off). `gfx` writes the graphics (gfx.rs; None
/// in tests and for builds without settings).
pub fn switch(
    client: &Path,
    id: &str,
    gfx: Option<&dyn Fn(&str) -> Result<(), String>>,
) -> Result<Switched, String> {
    let mut book = load(client);
    let man = mods::load(client);
    let (items, name, want_gfx) = if id == CLEAN {
        (Vec::new(), "Clean".to_string(), String::new())
    } else {
        let l = book.get(id).ok_or("That loadout isn't there any more.")?;
        (l.items.clone(), l.name.clone(), l.gfx.clone())
    };
    sync_active(&mut book, &man);
    // a copy of the settings of the loadout being left (or of the game as it was, the first time)
    let left = if book.active.is_empty() {
        "before-loadouts".to_string()
    } else {
        book.active.clone()
    };
    let rel = format!("{}/{}/{}/WTF", mods::DIR, BACKUP_DIR, left);
    let copied = backup_wtf(client, &client.join(&rel))?;
    let p = plan(&man, &items);
    let mut out = Switched {
        backup: if copied > 0 { rel } else { String::new() },
        missing: p.missing.clone(),
        ..Default::default()
    };
    // off first, so folders are free for what goes back on
    for k in &p.disable {
        match mods::set_enabled(client, k, false) {
            Ok(()) => out.disabled.push(k.clone()),
            Err(e) => out.problems.push(format!("{k}: {e}")),
        }
    }
    for k in &p.enable {
        match mods::set_enabled(client, k, true) {
            Ok(()) => out.enabled.push(k.clone()),
            Err(e) => out.problems.push(format!("{k}: {e}")),
        }
    }
    if let Some(f) = gfx {
        let what = if id == CLEAN {
            "original"
        } else {
            want_gfx.as_str()
        };
        if !what.is_empty() {
            out.gfx = match f(what) {
                Ok(()) => {
                    if id == CLEAN {
                        "original".into()
                    } else {
                        "preset".into()
                    }
                }
                Err(e) => e,
            };
        }
    }
    if book.active != id {
        book.previous = book.active.clone();
    }
    book.active = id.to_string();
    out.active = book.active.clone();
    out.previous = book.previous.clone();
    out.name = name;
    save(client, &book)?;
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ the status

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct InstalledItem {
    pub key: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub client: String,
    pub active: String,
    pub previous: String,
    pub loadouts: Vec<Loadout>,
    /// Every mod Leyline installed here, on or off.
    pub installed: Vec<InstalledItem>,
    /// The loadout that is on no longer matches what is on (only on Clean: mods were turned on or installed there).
    pub drift: Vec<String>,
}

pub fn status(client: &Path, client_id: &str) -> Status {
    let mut book = load(client);
    let man = mods::load(client);
    sync_active(&mut book, &man);
    let drift = if book.active == CLEAN {
        enabled_now(&man)
    } else {
        Vec::new()
    };
    Status {
        client: client_id.to_string(),
        active: book.active.clone(),
        previous: book.previous.clone(),
        loadouts: book.loadouts.clone(),
        installed: man
            .mods
            .iter()
            .map(|m| InstalledItem {
                key: m.key.clone(),
                name: m.name.clone(),
                enabled: m.enabled,
            })
            .collect(),
        drift,
    }
}

#[cfg(all(feature = "app", desktop))] // desktop only: the phone app has its own short list (mobile.rs)
pub mod commands {
    use super::*;
    use tauri::State as TState;

    type Core<'a> = TState<'a, crate::app::Core>;

    fn dir(
        core: &crate::app::Core,
        client: &str,
    ) -> Result<(crate::clients::ClientProfile, PathBuf), String> {
        let c = core.client(client)?;
        let p = c.path.clone();
        Ok((c, p))
    }

    #[tauri::command]
    pub fn loadouts_status(core: Core, client: String) -> Result<Status, String> {
        let (_, p) = dir(&core, &client)?;
        Ok(status(&p, &client))
    }
    #[tauri::command]
    pub fn loadout_new(
        core: Core,
        client: String,
        name: String,
        empty: bool,
    ) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        add(
            &p,
            &name,
            if empty { Some(Vec::new()) } else { None },
            "",
            "",
        )
    }
    #[tauri::command]
    pub fn loadout_add_shared(
        core: Core,
        client: String,
        shared: Shared,
        from: String,
    ) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        add_shared(&p, &shared, &from)
    }
    #[tauri::command]
    pub fn loadout_import(core: Core, client: String, code: String) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        let s = parse_code(&code)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        add_shared(&p, &s, "code")
    }
    #[tauri::command]
    pub fn loadout_code(core: Core, client: String, id: String) -> Result<String, String> {
        let (_, p) = dir(&core, &client)?;
        let mut book = load(&p);
        sync_active(&mut book, &mods::load(&p));
        book.get(&id)
            .map(share_code)
            .ok_or_else(|| "That loadout isn't there any more.".into())
    }
    #[tauri::command]
    pub fn loadout_copy(
        core: Core,
        client: String,
        id: String,
        name: String,
    ) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        copy(&p, &id, &name)
    }
    #[tauri::command]
    pub fn loadout_rename(
        core: Core,
        client: String,
        id: String,
        name: String,
    ) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        rename(&p, &id, &name)
    }
    #[tauri::command]
    pub fn loadout_set_gfx(
        core: Core,
        client: String,
        id: String,
        gfx: String,
    ) -> Result<Loadout, String> {
        let (_, p) = dir(&core, &client)?;
        set_gfx(&p, &id, &gfx)
    }
    #[tauri::command]
    pub fn loadout_delete(core: Core, client: String, id: String) -> Result<(), String> {
        let (_, p) = dir(&core, &client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        delete(&p, &id)
    }
    #[tauri::command]
    pub fn loadout_switch(core: Core, client: String, id: String) -> Result<Switched, String> {
        let (c, p) = dir(&core, &client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        let write_gfx = |what: &str| -> Result<(), String> {
            if crate::gfx::tweaks(c.build).is_empty() {
                return Ok(());
            }
            if what == "original" {
                crate::gfx::back_to_original(&c).map(|_| ())
            } else {
                crate::gfx::preset(&c, what).map(|_| ())
            }
        };
        switch(&p, &id, Some(&write_gfx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::Installed;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    /// A game folder with two Leyline mods (Skada and Dominos, both on), a game file and an addon the player put in.
    fn folder() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path();
        put(r, "Data/common.MPQ", "");
        put(r, "Interface/AddOns/Mine/Mine.toc", "## Interface: 30300\n");
        put(r, "WTF/Config.wtf", "SET farclip \"777\"\n");
        put(r, "WTF/Account/A/bindings-cache.wtf", "bind\n");
        let mut man = Manifest {
            schema: 1,
            build: "3.3.5a".into(),
            ..Default::default()
        };
        for (key, addon) in [("up:skada", "Skada"), ("up:dominos", "Dominos")] {
            let f = format!("Interface/AddOns/{addon}/{addon}.toc");
            put(r, &f, "## Interface: 30300\n");
            man.mods.push(Installed {
                key: key.into(),
                name: addon.into(),
                source: "catalog".into(),
                enabled: true,
                addons: vec![addon.into()],
                files: vec![f],
                dirs: vec![format!("Interface/AddOns/{addon}")],
                ..Default::default()
            });
        }
        put(
            r,
            "Leyline/mods.json",
            &serde_json::to_string(&man).unwrap(),
        );
        d
    }

    #[test]
    fn names_and_ids_are_checked() {
        assert_eq!(clean_name("  Raider\u{7}  ").as_deref(), Some("Raider"));
        assert_eq!(clean_name("   "), None);
        assert_eq!(
            clean_name(&"x".repeat(90)).unwrap().chars().count(),
            MAX_NAME
        );
        assert!(
            shareable("up:skada")
                && shareable("mod:pings")
                && shareable("look:high")
                && shareable("leyline:addons")
        );
        assert!(
            !shareable("local:Mine")
                && !shareable("https://x.example/a.zip")
                && !shareable("up:../x")
        );
        assert!(item_ok("local:Mine") && item_ok("patch:Data/patch-4.MPQ") && !item_ok("bad"));
        let b = Book {
            loadouts: vec![Loadout {
                id: "raider".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert_eq!(new_id(&b, "Raider!"), "raider-2");
        assert_eq!(new_id(&b, "Clean"), "loadout");
        assert_eq!(new_id(&b, "My friend's realm"), "my-friend-s-realm");
    }

    #[test]
    fn a_share_code_carries_catalog_ids_only_and_reads_back() {
        let l = Loadout {
            name: "Raid; night & more".into(),
            items: vec![
                "up:skada".into(),
                "local:Mine".into(),
                "leyline:addons".into(),
            ],
            gfx: "high".into(),
            ..Default::default()
        };
        let code = share_code(&l);
        assert!(code.starts_with(CODE_PREFIX));
        assert!(
            !code.contains("local:"),
            "a mod from the player's own PC is not in a code"
        );
        let s = parse_code(&code).unwrap();
        assert_eq!(s.name, "Raid; night & more");
        assert_eq!(
            s.items,
            vec!["up:skada".to_string(), "leyline:addons".to_string()]
        );
        assert_eq!(s.gfx, "high");
        // anything that is not a catalog id is dropped; an unknown preset is left out
        let s = parse_code("leyline-loadout:1;n=X;g=insane;i=up:skada,https://evil.example/x.zip,up:skada,file:C:/x").unwrap();
        assert_eq!(s.items, vec!["up:skada".to_string()]);
        assert_eq!(s.gfx, "");
        assert!(parse_code("hello").is_err());
        assert!(parse_code(&format!("{CODE_PREFIX}n={}", "a".repeat(MAX_CODE))).is_err());
    }

    #[test]
    fn the_plan_turns_off_what_is_not_in_the_loadout_and_names_what_is_missing() {
        let d = folder();
        let man = mods::load(d.path());
        let p = plan(
            &man,
            &["up:skada".into(), "up:questie".into(), "local:Gone".into()],
        );
        assert_eq!(p.disable, vec!["up:dominos".to_string()]);
        assert!(p.enable.is_empty());
        assert_eq!(
            p.missing,
            vec!["up:questie".to_string()],
            "only catalog ids can be installed"
        );
        assert_eq!(plan(&man, &[]).disable.len(), 2);
    }

    #[test]
    fn clean_and_back_never_touch_game_files_or_the_players_own_addons() {
        let d = folder();
        let r = d.path();
        let raider = add(r, "Raider", None, "", "").unwrap();
        assert_eq!(
            raider.items,
            vec!["up:skada".to_string(), "up:dominos".to_string()],
            "what is on now"
        );
        let s = switch(r, CLEAN, None).unwrap();
        assert_eq!(s.disabled.len(), 2);
        assert!(s.problems.is_empty());
        assert!(
            !r.join("Interface/AddOns/Skada").exists(),
            "turned off: out of the game's sight"
        );
        assert!(r.join("Leyline/disabled").exists());
        assert!(
            r.join("Interface/AddOns/Mine/Mine.toc").is_file(),
            "the player's own addon stays"
        );
        assert!(r.join("Data/common.MPQ").is_file(), "game files stay");
        assert!(
            r.join(&s.backup).join("Config.wtf").is_file(),
            "a WTF copy first"
        );
        assert!(r
            .join(&s.backup)
            .join("Account/A/bindings-cache.wtf")
            .is_file());
        assert_eq!(status(r, "c1").active, CLEAN);
        // and back, one click
        let s = switch(r, &raider.id, None).unwrap();
        assert_eq!(s.enabled.len(), 2);
        assert_eq!(s.previous, CLEAN);
        assert!(r.join("Interface/AddOns/Skada/Skada.toc").is_file());
        assert!(r.join("Interface/AddOns/Dominos/Dominos.toc").is_file());
    }

    #[test]
    fn the_loadout_that_is_on_follows_the_player_and_others_keep_their_own() {
        let d = folder();
        let r = d.path();
        let a = add(r, "Raider", None, "", "").unwrap();
        let b = add_shared(
            r,
            &Shared {
                name: "Quester".into(),
                items: vec!["up:skada".into(), "up:questie".into()],
                gfx: "".into(),
            },
            "template:quester",
        )
        .unwrap();
        switch(r, &a.id, None).unwrap();
        mods::set_enabled(r, "up:dominos", false).unwrap(); // the player turns Dominos off while Raider is on
        let s = switch(r, &b.id, None).unwrap();
        assert_eq!(s.missing, vec!["up:questie".to_string()]);
        assert!(s.disabled.is_empty(), "Dominos was already off");
        let st = status(r, "c1");
        assert_eq!(
            st.loadouts.iter().find(|l| l.id == a.id).unwrap().items,
            vec!["up:skada".to_string()],
            "Raider remembered it"
        );
        assert_eq!(st.active, b.id);
        assert_eq!(st.previous, a.id);
        // a copy, a rename, a preset, a delete (never the one that is on)
        let c = copy(r, &b.id, "Quester 2").unwrap();
        assert_eq!(
            c.items,
            vec!["up:skada".to_string(), "up:questie".to_string()]
        );
        assert_eq!(rename(r, &c.id, "Alt").unwrap().name, "Alt");
        assert_eq!(set_gfx(r, &c.id, "high").unwrap().gfx, "high");
        assert!(set_gfx(r, &c.id, "turbo").is_err());
        assert!(delete(r, &b.id).is_err());
        delete(r, &c.id).unwrap();
        assert!(status(r, "c1").loadouts.iter().all(|l| l.id != c.id));
    }

    #[test]
    fn clean_shows_mods_turned_on_while_it_is_on() {
        let d = folder();
        let r = d.path();
        switch(r, CLEAN, None).unwrap();
        mods::set_enabled(r, "up:skada", true).unwrap();
        assert_eq!(status(r, "c1").drift, vec!["up:skada".to_string()]);
    }

    #[test]
    fn graphics_go_with_the_loadout_and_clean_asks_for_the_original() {
        let d = folder();
        let r = d.path();
        let l = add_shared(
            r,
            &Shared {
                name: "HD looks".into(),
                items: vec![],
                gfx: "ultra".into(),
            },
            "",
        )
        .unwrap();
        let seen = std::cell::RefCell::new(Vec::new());
        let f = |w: &str| -> Result<(), String> {
            seen.borrow_mut().push(w.to_string());
            Ok(())
        };
        assert_eq!(switch(r, &l.id, Some(&f)).unwrap().gfx, "preset");
        assert_eq!(switch(r, CLEAN, Some(&f)).unwrap().gfx, "original");
        assert_eq!(
            *seen.borrow(),
            vec!["ultra".to_string(), "original".to_string()]
        );
    }

    #[test]
    fn a_damaged_file_or_too_many_loadouts_is_handled() {
        let d = folder();
        let r = d.path();
        put(r, "Leyline/loadouts.json", "{not json");
        assert_eq!(load(r), Book::default());
        put(
            r,
            "Leyline/loadouts.json",
            r#"{"active":"gone","loadouts":[{"id":"clean","name":"x"},{"id":"a","name":"A","items":["up:skada","bad id"],"gfx":"turbo"}]}"#,
        );
        let b = load(r);
        assert_eq!(b.active, "");
        assert_eq!(b.loadouts.len(), 1);
        assert_eq!(b.loadouts[0].items, vec!["up:skada".to_string()]);
        assert_eq!(b.loadouts[0].gfx, "");
        for i in 0..MAX_LOADOUTS - 1 {
            add(r, &format!("L{i}"), Some(vec![]), "", "").unwrap();
        }
        assert!(add(r, "One too many", Some(vec![]), "", "").is_err());
    }
}
