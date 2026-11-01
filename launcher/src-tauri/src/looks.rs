//! Looks (Mods › Discover › Looks; owner 1 Nov 14:38: "make the graphical upgrades and changes a main part ... 1 click
//! install on them for most clients"): graphics packs, UI looks and graphics presets, from the catalog's `source: "look"`
//! entries (`stream/looks.json`, mods/SPEC.md "Looks"). The website's /wrath/looks lists the same entries and its
//! "Install with Leyline" opens `leyline://install/look:<slug>` here.
//!
//! What one click does, by what the entry is:
//! * a preset (`look.preset`): the game's own settings, through gfx.rs (a copy of Config.wtf is kept first);
//! * a graphics pack with an install block: the author's own GitHub release file (a `.MPQ`, or a zip of them), checked
//!   against its pinned sha256 and every file's sha256 (`install_ok`, the listed mods' download and unpack), then each
//!   patch into a free `Data/patch-<X>.MPQ` slot through the mod manager, as `look:<slug>` (never T or Z: mods.rs);
//! * a UI look (`look.uses`): each curated addon from its author (upstream.rs), only when every one of them is reviewed
//!   and in the catalog. The addons the look put in are remembered (`STATE_FILE`, in the launcher's own folder), so
//!   removing the look takes out exactly those and never an addon the player had already;
//! * anything else is a link to the author's page (the UI opens it).
//!
//! "Back to the original look" (`reset`): every look Leyline put in that client comes out (packs and the look's addons),
//! and Config.wtf goes back to the copy kept before Leyline first changed it. Nothing is ever hosted by us.

use crate::catalog::{text, Project};
use crate::clients::{Build, ClientProfile};
use crate::mods::{self, InstallMeta, Installed};
use crate::upstream::{self, Install};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// In the launcher's own folder: which addons each UI look installed, per client folder.
pub const STATE_FILE: &str = "looks-applied.json";
pub const TYPES: &[&str] = &["graphics-pack", "ui-look", "preset"];

/// The `look` object of a catalog entry (mods/all-json.md "Looks").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    /// `graphics-pack`, `ui-look` or `preset`.
    #[serde(rename = "type")]
    pub kind: String,
    /// A preset: the gfx.rs preset id.
    pub preset: String,
    /// A UI look: the curated addons (`up:<slug>`) it installs.
    pub uses: Vec<String>,
    pub changes: Vec<String>,
    pub featured: bool,
    pub order: i64,
    pub note: String,
    /// What the site worked out (`launcher` or `link`); the launcher works it out again (`how`).
    pub how: String,
}

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// A graphics pack's install block: the listed mods' `files` rules (listed::install_ok), plus: a release file of the
/// entry's own repo (byte-stable), its sha256 pinned, and only client patches (`.MPQ`) in it.
pub fn install_ok(inst: &Install, repo: &str) -> bool {
    inst.kind == "files"
        && inst.from == "release-asset"
        && inst.sha256.as_deref().is_some_and(hex64)
        && !inst.files.is_empty()
        && inst.files.len() <= 16
        && inst.files.keys().all(|f| f.to_ascii_lowercase().ends_with(".mpq"))
        && crate::listed::install_ok(inst, repo, false)
}

/// The look fields made safe (catalog::clean). Not a look: `look` is dropped. A look's install block that does not
/// check out is dropped (it is a link then); uses keep only curated addon ids; a preset Leyline does not have is none.
pub fn clean(p: &mut Project) {
    if p.source != "look" {
        p.look = None;
        return;
    }
    p.download = None;
    p.mirror_allowed = false;
    p.addons.clear();
    p.requires.clear();
    p.conflicts.clear();
    p.review = None;
    p.compat = None;
    p.native = false;
    p.name = upstream::clean_title(&p.name);
    if p.name.is_empty() {
        p.name = p.id[5..].to_string();
    }
    let Some(mut lk) = p.look.take() else {
        p.source = "community".into();
        return;
    };
    if !TYPES.contains(&lk.kind.as_str()) {
        p.source = "community".into();
        p.install = None;
        return;
    }
    if lk.kind != "preset" || !crate::gfx::PRESETS.iter().any(|(id, _, _)| *id == lk.preset) {
        lk.preset.clear();
    }
    lk.uses = std::mem::take(&mut lk.uses)
        .into_iter()
        .filter(|u| u.starts_with("up:") && crate::catalog::id_ok(u))
        .take(8)
        .collect();
    if lk.kind != "ui-look" {
        lk.uses.clear();
    }
    lk.changes = std::mem::take(&mut lk.changes).into_iter().map(|c| text(&c, 32)).filter(|c| !c.is_empty()).take(12).collect();
    lk.note = text(&lk.note, 240);
    lk.how = if lk.how == "launcher" { "launcher".into() } else { "link".into() };
    let ok = lk.kind == "graphics-pack" && p.install.as_ref().is_some_and(|i| install_ok(i, &p.repo));
    if !ok {
        p.install = None;
    }
    if let Some(i) = &p.install {
        p.size = Some(i.size);
        p.file_count = Some(i.file_count);
        p.sha256 = i.sha256.clone();
    }
    p.look = Some(lk);
}

/// What one click does with this look here: `preset`, `pack`, `addons`, or `link` (the author's page). A UI look is
/// one click only when every addon it uses is a reviewed, installable entry of this catalog.
pub fn how(p: &Project, catalog: &[Project]) -> &'static str {
    let Some(lk) = p.look.as_ref().filter(|_| p.source == "look") else { return "link" };
    match lk.kind.as_str() {
        "preset" if !lk.preset.is_empty() => "preset",
        "graphics-pack" if p.install.is_some() => "pack",
        "ui-look" if !lk.uses.is_empty() && lk.uses.iter().all(|u| addon_of(catalog, u).is_some()) => "addons",
        _ => "link",
    }
}

fn addon_of<'a>(catalog: &'a [Project], id: &str) -> Option<&'a Project> {
    catalog.iter().find(|x| x.id == id && x.source == "upstream" && x.install.is_some())
}

// ------------------------------------------------------------------------------------------------ the remembered addons

/// `{client folder: {look id: [addon keys the look put in]}}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Applied {
    pub clients: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

fn state_path(app_dir: &Path) -> PathBuf {
    app_dir.join(STATE_FILE)
}

fn client_key(client: &Path) -> String {
    client.display().to_string()
}

pub fn load_applied(app_dir: &Path) -> Applied {
    fs::read(state_path(app_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_applied(app_dir: &Path, a: &Applied) -> Result<(), String> {
    fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
    let tmp = state_path(app_dir).with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(a).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&tmp, state_path(app_dir)).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------------------------ one click

/// What a click did, for the UI's status line.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Done {
    pub id: String,
    pub name: String,
    /// `preset`, `pack` or `addons`.
    pub how: String,
    pub said: String,
    pub installed: Vec<String>,
    pub warnings: Vec<String>,
}

/// Install (or use) a look in a client. `install_addon` puts one curated addon in (upstream::install in the app; a
/// stand-in in the tests); `install_pack` downloads and installs a pack (`install_pack`, checked addresses).
pub fn apply(
    c: &ClientProfile,
    app_dir: &Path,
    p: &Project,
    catalog: &[Project],
    install_pack: &dyn Fn(&Path, Build, &Project) -> Result<Installed, String>,
    install_addon: &dyn Fn(&Path, Build, &Project) -> Result<Installed, String>,
) -> Result<Done, String> {
    if p.source != "look" {
        return Err("That is not a look.".into());
    }
    if !p.fits(c.build) {
        return Err(format!("{} is made for another game version, not this one.", p.name));
    }
    let lk = p.look.clone().unwrap_or_default();
    let mut done = Done { id: p.id.clone(), name: p.name.clone(), how: how(p, catalog).into(), ..Default::default() };
    match done.how.as_str() {
        "preset" => {
            crate::gfx::preset(c, &lk.preset)?;
            done.said = format!("{} is on. It takes effect the next time the game starts.", p.name);
        }
        "pack" => {
            let m = install_pack(&c.path, c.build, p)?;
            done.installed = m.patches.clone();
            done.warnings = m.warnings.clone();
            done.said = format!("{} is in. It shows the next time the game starts.", p.name);
        }
        "addons" => {
            let have: Vec<String> = mods::load(&c.path).mods.iter().map(|m| m.key.clone()).collect();
            let mut put = Vec::new();
            for id in &lk.uses {
                if have.contains(id) {
                    continue; // the player has it already: it stays theirs
                }
                let a = addon_of(catalog, id).ok_or("One of this look's addons isn't ready for one click yet.")?;
                match install_addon(&c.path, c.build, a) {
                    Ok(m) => {
                        done.warnings.extend(m.warnings.clone());
                        put.push(id.clone());
                    }
                    Err(e) => {
                        for k in &put {
                            let _ = mods::uninstall(&c.path, k); // all or nothing
                        }
                        return Err(format!("{} could not go in, so nothing was changed: {e}", a.name));
                    }
                }
            }
            let mut st = load_applied(app_dir);
            let mine = st.clients.entry(client_key(&c.path)).or_default().entry(p.id.clone()).or_default();
            for k in &put {
                if !mine.contains(k) {
                    mine.push(k.clone());
                }
            }
            save_applied(app_dir, &st)?;
            done.installed = put;
            done.said = format!("{} is in. It shows the next time the game starts (or after /reload in game).", p.name);
        }
        _ => return Err(format!("{} installs from its author's page: open it there.", p.name)),
    }
    Ok(done)
}

/// Download a graphics pack from its author and install it (key `look:<slug>`): the address, the release file's sha256
/// and every file's sha256 are checked before the client folder is touched.
pub fn install_pack(client: &Path, build: Build, p: &Project, scratch: &Path) -> Result<Installed, String> {
    let inst = p.install.as_ref().filter(|i| p.source == "look" && install_ok(i, &p.repo)).ok_or("This look's download details did not check out, so Leyline will not install it.")?;
    install_pack_from(client, build, p, inst, scratch)
}

/// `install_pack` after the address checks (the tests serve the file on 127.0.0.1).
pub fn install_pack_from(client: &Path, build: Build, p: &Project, inst: &Install, scratch: &Path) -> Result<Installed, String> {
    let dl = mods::Scratch::new(scratch).map_err(|e| format!("could not make a download folder: {e}"))?;
    let file = dl.dir.join("look.download");
    let max = (inst.size.saturating_mul(2) + 16 * 1_048_576).min(mods::MAX_PATCH + 64 * 1_048_576);
    let (_, sha) = upstream::download(&inst.url, &file, max).map_err(|e| format!("Download from the author's site failed: {e}"))?;
    if inst.sha256.as_deref() != Some(sha.as_str()) {
        return Err("The download does not match the file that was checked, so it was not installed. The author may have changed it: try again later.".into());
    }
    let st = mods::Staging::new(client).map_err(|e| format!("could not make a staging folder in the client: {e}"))?;
    let files = st.files();
    fs::create_dir_all(&files).map_err(|e| e.to_string())?;
    let u = crate::listed::unpack_files(&file, &files, inst)?;
    upstream::verify(&u, inst)?;
    let plan = mods::validate(&mods::walk(&files).map_err(|e| e.to_string())?, build);
    if !plan.errors.is_empty() {
        return Err(format!("{} can't go into this client: {}", p.name, plan.errors.join("; ")));
    }
    if plan.patches.is_empty() || !plan.addons.is_empty() || !plan.natives.is_empty() {
        return Err(format!("{} is not only client patches, so Leyline will not install it as a look.", p.name));
    }
    let meta = InstallMeta {
        key: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone().unwrap_or_else(|| inst.rev.chars().take(16).collect()),
        source: "look".into(),
        sha256: inst.rev.clone(),
        category: p.category.clone(),
        credits: p.credits.clone(),
    };
    mods::install(client, build, meta, &plan, false)
}

/// The update an installed pack has: the catalog's release tag when it differs.
pub fn update_for(m: &Installed, p: &Project) -> Option<String> {
    let inst = p.install.as_ref()?;
    (p.source == "look" && m.source == "look" && !inst.rev.is_empty() && inst.rev != m.sha256).then(|| inst.rev.chars().take(16).collect())
}

/// Take one look out: a pack's files, or the addons the look put in. A preset has nothing to take out (Back to the
/// original look puts the settings back).
pub fn remove(c: &ClientProfile, app_dir: &Path, id: &str) -> Result<usize, String> {
    let mut n = 0;
    if mods::load(&c.path).get(id).is_some() {
        mods::uninstall(&c.path, id)?;
        n += 1;
    }
    let mut st = load_applied(app_dir);
    if let Some(keys) = st.clients.get_mut(&client_key(&c.path)).and_then(|m| m.remove(id)) {
        let man = mods::load(&c.path);
        for k in keys {
            if man.get(&k).is_some() {
                mods::uninstall(&c.path, &k)?;
                n += 1;
            }
        }
        st.clients.retain(|_, m| !m.is_empty());
        save_applied(app_dir, &st)?;
    }
    Ok(n)
}

/// What "Back to the original look" did.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Reset {
    pub removed: usize,
    pub settings_restored: bool,
    pub said: String,
}

/// Back to the original look: every look Leyline put in this client comes out, and the game's settings go back to
/// the copy kept before Leyline first changed them.
pub fn reset(c: &ClientProfile, app_dir: &Path) -> Result<Reset, String> {
    let mut out = Reset::default();
    let ids: Vec<String> = load_applied(app_dir).clients.get(&client_key(&c.path)).map(|m| m.keys().cloned().collect()).unwrap_or_default();
    for id in ids {
        out.removed += remove(c, app_dir, &id)?;
    }
    let packs: Vec<String> = mods::load(&c.path).mods.iter().filter(|m| m.key.starts_with("look:")).map(|m| m.key.clone()).collect();
    for k in packs {
        mods::uninstall(&c.path, &k)?;
        out.removed += 1;
    }
    if crate::gfx::read(c).has_backup {
        crate::gfx::restore(c)?;
        out.settings_restored = true;
    }
    out.said = match (out.removed, out.settings_restored) {
        (0, false) => "This game already has its original look.".into(),
        (0, true) => "Back to the original look: the game's settings are as they were before Leyline changed them.".into(),
        (n, s) => format!(
            "Back to the original look: {n} look{} taken out{}.",
            if n == 1 { "" } else { "s" },
            if s { ", and the game's settings are as they were before Leyline changed them" } else { "" }
        ),
    };
    Ok(out)
}

/// What the Looks tab shows for one client.
#[derive(Debug, Clone, Serialize)]
pub struct State {
    pub client: String,
    pub build: Build,
    /// The look ids that are in (packs, UI looks) or in use (the preset whose values the settings have now).
    pub on: Vec<String>,
    /// look id -> `preset`, `pack`, `addons` or `link` for this catalog.
    pub how: BTreeMap<String, String>,
    /// Leyline changed something a reset would undo.
    pub changed: bool,
}

pub fn state(c: &ClientProfile, app_dir: &Path, catalog: &[Project]) -> State {
    let man = mods::load(&c.path);
    let applied = load_applied(app_dir);
    let mine = applied.clients.get(&client_key(&c.path)).cloned().unwrap_or_default();
    let g = crate::gfx::read(c);
    let mut on = Vec::new();
    let mut hows = BTreeMap::new();
    for p in catalog.iter().filter(|p| p.source == "look") {
        let h = how(p, catalog);
        hows.insert(p.id.clone(), h.to_string());
        let is_on = match h {
            "preset" => preset_on(&g, &p.look.as_ref().map(|l| l.preset.clone()).unwrap_or_default()),
            _ => man.get(&p.id).is_some() || mine.get(&p.id).is_some_and(|ks| ks.iter().any(|k| man.get(k).is_some())),
        };
        if is_on {
            on.push(p.id.clone());
        }
    }
    let changed = g.has_backup || man.mods.iter().any(|m| m.key.starts_with("look:")) || !mine.is_empty();
    State { client: c.name.clone(), build: c.build, on, how: hows, changed }
}

/// The settings now are this preset's (for the settings this game version has).
fn preset_on(g: &crate::gfx::State, id: &str) -> bool {
    let Some((_, _, vals)) = crate::gfx::PRESETS.iter().find(|(p, _, _)| *p == id) else { return false };
    if g.tweaks.is_empty() {
        return false;
    }
    let mut any = false;
    for (k, v) in vals.iter() {
        let Some(t) = g.tweaks.iter().find(|t| t.key == *k) else { continue };
        let want = if t.kind == "slider" {
            let n: f64 = v.parse().unwrap_or(0.0);
            let n = n.clamp(t.min, t.max);
            if t.step >= 1.0 { format!("{}", n.round() as i64) } else { format!("{n:.1}") }
        } else {
            v.to_string()
        };
        let have = g.values.get(*k).cloned().unwrap_or_default();
        let same = match (have.parse::<f64>(), want.parse::<f64>()) {
            (Ok(a), Ok(b)) => (a - b).abs() < 1e-6,
            _ => have == want,
        };
        if !same {
            return false;
        }
        any = true;
    }
    any
}

#[cfg(feature = "app")]
pub mod commands {
    use super::*;
    use tauri::State as TState;

    type Core<'a> = TState<'a, crate::app::Core>;

    fn catalog(core: &crate::app::Core) -> Vec<Project> {
        crate::catalog::cached(&core.dir, crate::catalog::URL).unwrap_or_default()
    }

    #[tauri::command]
    pub fn looks_state(core: Core, client: String) -> Result<State, String> {
        let c = core.client(&client)?;
        Ok(state(&c, &core.dir, &catalog(&core)))
    }

    #[tauri::command]
    pub async fn looks_apply(core: Core<'_>, client: String, look: String) -> Result<Done, String> {
        let c = core.client(&client)?;
        let cat = catalog(&core);
        let p = cat.iter().find(|p| p.id == look).cloned().ok_or("That look is not in the list any more.")?;
        let (dir, lock) = (core.dir.clone(), core.mods_lock.clone());
        let origin = crate::net::origin(crate::catalog::URL).unwrap_or_default();
        tauri::async_runtime::spawn_blocking(move || {
            let _g = lock.lock().map_err(|_| "mod lock poisoned")?;
            let scratch = dir.clone();
            apply(
                &c,
                &dir,
                &p,
                &cat,
                &|client, build, p| install_pack(client, build, p, &scratch),
                &|client, build, a| upstream::install(client, build, a, &origin, &scratch),
            )
        })
        .await
        .map_err(|e| e.to_string())?
    }

    #[tauri::command]
    pub fn looks_remove(core: Core, client: String, look: String) -> Result<usize, String> {
        let c = core.client(&client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        remove(&c, &core.dir, &look)
    }

    #[tauri::command]
    pub fn looks_reset(core: Core, client: String) -> Result<Reset, String> {
        let c = core.client(&client)?;
        let _g = core.mods_lock.lock().map_err(|_| "mod lock poisoned")?;
        reset(&c, &core.dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::testutil::fake_335a;
    use crate::mods::tests::zip_of;
    use crate::net::testserver::serve_routes;

    fn sha(b: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        crate::net::hex(&Sha256::digest(b))
    }
    const MPQ: &[u8] = b"MPQ\x1a\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
    const MPQ2: &[u8] = b"MPQ\x1a\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

    fn client(dir: &Path) -> ClientProfile {
        fake_335a(dir, false);
        serde_json::from_value(serde_json::json!({"id":"c1","name":"Mine","path":dir,"build":"3.3.5a"})).unwrap()
    }

    fn block(url: &str, files: &[(&str, &[u8])], prefix: &str, whole: &[u8]) -> Install {
        let f: BTreeMap<String, String> = files.iter().map(|(p, b)| (p.to_string(), sha(b))).collect();
        Install {
            kind: "files".into(),
            from: "release-asset".into(),
            url: url.into(),
            rev: "v1".into(),
            sha256: Some(sha(whole)),
            size: whole.len() as u64,
            file_count: files.len() as u64,
            strip_prefix: prefix.into(),
            files_digest: upstream::files_digest(&f),
            files: f,
            ..Default::default()
        }
    }

    fn look(id: &str, kind: &str, preset: &str, uses: &[&str], inst: Option<Install>) -> Project {
        Project {
            id: id.into(),
            source: "look".into(),
            name: id[5..].into(),
            builds: vec!["3.3.5a".into(), "1.12".into()],
            repo: "https://github.com/alice/water".into(),
            install: inst,
            look: Some(Look { kind: kind.into(), preset: preset.into(), uses: uses.iter().map(|s| s.to_string()).collect(), ..Default::default() }),
            ..Default::default()
        }
    }

    fn addon(id: &str) -> Project {
        Project {
            id: id.into(),
            source: "upstream".into(),
            name: id[3..].into(),
            builds: vec!["3.3.5a".into()],
            install: Some(Install::default()),
            ..Default::default()
        }
    }

    /// A stand-in for upstream::install: one addon folder named after the id, through the mod manager.
    fn fake_addon(client: &Path, build: Build, a: &Project) -> Result<Installed, String> {
        let name: String = a.id[3..].split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
        let st = mods::Staging::new(client).unwrap();
        let files = st.files();
        let dir = files.join(&name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{name}.toc")), format!("## Interface: 30300\n## Title: {name}\n{name}.lua\n")).unwrap();
        fs::write(dir.join(format!("{name}.lua")), "x = 1\n").unwrap();
        let plan = mods::validate(&mods::walk(&files).unwrap(), build);
        assert!(plan.errors.is_empty(), "{:?}", plan.errors);
        let meta = InstallMeta { key: a.id.clone(), name: a.name.clone(), source: "upstream".into(), ..Default::default() };
        mods::install(client, build, meta, &plan, false)
    }

    fn no_pack(_: &Path, _: Build, _: &Project) -> Result<Installed, String> {
        Err("no pack here".into())
    }

    #[test]
    fn the_catalog_keeps_looks_and_works_out_how_each_installs() {
        let v = serde_json::json!({"projects": [
            {"id": "look:max", "source": "look", "name": "Max", "builds": ["1.12", "3.3.5a"], "look": {"type": "preset", "preset": "ultra", "how": "launcher"}},
            {"id": "look:fake", "source": "look", "name": "Fake", "look": {"type": "preset", "preset": "insane"}},
            {"id": "look:water", "source": "look", "name": "Water", "repo": "https://github.com/alice/water",
             "look": {"type": "graphics-pack"},
             "install": block("https://github.com/alice/water/releases/download/v1/patch-W.MPQ", &[("patch-W.MPQ", MPQ)], "", MPQ)},
            {"id": "look:evil", "source": "look", "name": "Evil", "repo": "https://github.com/alice/water",
             "look": {"type": "graphics-pack"},
             "install": block("https://github.com/mallory/water/releases/download/v1/patch-W.MPQ", &[("patch-W.MPQ", MPQ)], "", MPQ)},
            {"id": "look:dll", "source": "look", "name": "Dll", "repo": "https://github.com/alice/water",
             "look": {"type": "graphics-pack"},
             "install": block("https://github.com/alice/water/releases/download/v1/pack.zip", &[("d3d9.dll", b"MZ")], "", b"PK")},
            {"id": "look:frames", "source": "look", "name": "Frames", "look": {"type": "ui-look", "uses": ["up:ruf", "mod:x"]}},
            {"id": "look:noobj", "source": "look", "name": "No look object"},
            {"id": "up:ruf", "source": "upstream", "name": "RUF"}
        ]});
        let (_, ps) = crate::catalog::parse_in(&v, "https://play.openvanilla.app", true).unwrap();
        let by = |id: &str| ps.iter().find(|p| p.id == id).unwrap().clone();
        assert_eq!(by("look:max").source, "look");
        assert_eq!(how(&by("look:max"), &ps), "preset");
        assert_eq!(how(&by("look:fake"), &ps), "link", "a preset Leyline doesn't have is a link");
        assert_eq!(how(&by("look:water"), &ps), "pack");
        assert!(by("look:evil").install.is_none() && how(&by("look:evil"), &ps) == "link", "another repo's file");
        assert!(by("look:dll").install.is_none(), "only client patches in a look");
        assert_eq!(by("look:frames").look.unwrap().uses, vec!["up:ruf".to_string()]);
        assert_eq!(how(&by("look:frames"), &ps), "link", "up:ruf is not reviewed (no install): a link");
        assert_eq!(by("look:noobj").source, "community");
        assert!(by("up:ruf").look.is_none());
        let mut with = ps.clone();
        with.push(addon("up:ruf"));
        with.retain(|p| !(p.id == "up:ruf" && p.install.is_none()));
        assert_eq!(how(&by("look:frames"), &with), "addons");
        assert!(crate::deeplink::parse("leyline://install/look:max") == Some(crate::deeplink::Link::Install("look:max".into())));
    }

    #[test]
    fn a_preset_is_one_click_and_back_to_the_original_look_puts_the_settings_back() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let c = client(&d.path().join("game"));
        fs::create_dir_all(c.path.join("WTF")).unwrap();
        fs::write(c.path.join("WTF/Config.wtf"), "SET farclip \"477\"\r\nSET realmlist \"x\"\r\n").unwrap();
        let max = look("look:max", "preset", "ultra", &[], None);
        let classic = look("look:classic", "preset", "classic", &[], None);
        let cat = vec![max.clone(), classic.clone()];
        let s = state(&c, &app, &cat);
        assert!(s.on.is_empty() && !s.changed);
        let done = apply(&c, &app, &max, &cat, &no_pack, &fake_addon).unwrap();
        assert_eq!(done.how, "preset");
        assert_eq!(crate::gfx::read(&c).values["farclip"], "1277");
        let s = state(&c, &app, &cat);
        assert_eq!(s.on, vec!["look:max".to_string()], "Max is in use, Classic is not");
        assert!(s.changed);
        apply(&c, &app, &classic, &cat, &no_pack, &fake_addon).unwrap();
        assert_eq!(state(&c, &app, &cat).on, vec!["look:classic".to_string()]);
        let r = reset(&c, &app).unwrap();
        assert!(r.settings_restored && r.removed == 0, "{r:?}");
        let t = fs::read_to_string(c.path.join("WTF/Config.wtf")).unwrap();
        assert!(t.contains("farclip \"477\"") && t.contains("realmlist \"x\""), "{t}");
        assert!(!state(&c, &app, &cat).changed);
        assert_eq!(reset(&c, &app).unwrap().said, "This game already has its original look.");
        // made for another version: refused
        let mut old = max.clone();
        old.builds = vec!["1.12".into()];
        assert!(apply(&c, &app, &old, &cat, &no_pack, &fake_addon).unwrap_err().contains("another game version"));
    }

    #[test]
    fn a_pack_comes_from_the_authors_release_checked_into_a_free_slot_and_out_again() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let c = client(&d.path().join("game"));
        let zip = zip_of(&[("water-v1/patch-W.MPQ", MPQ), ("water-v1/Data/patch-X.MPQ", MPQ2), ("water-v1/README.txt", b"hi")]);
        let base = serve_routes(vec![("/alice/water/releases/download/v1/water.zip", "200 OK", zip.clone())]);
        let url = format!("{base}/alice/water/releases/download/v1/water.zip");
        let inst = block(&url, &[("patch-W.MPQ", MPQ), ("Data/patch-X.MPQ", MPQ2)], "water-v1/", &zip);
        let p = look("look:water", "graphics-pack", "", &[], Some(inst.clone()));
        let cat = vec![p.clone()];
        let scratch = d.path().join("scratch");
        fs::create_dir_all(&scratch).unwrap();
        let pack = |cl: &Path, b: Build, p: &Project| install_pack_from(cl, b, p, p.install.as_ref().unwrap(), &scratch);
        let done = apply(&c, &app, &p, &cat, &pack, &fake_addon).unwrap();
        assert_eq!(done.how, "pack");
        assert_eq!(done.installed.len(), 2, "{done:?}");
        assert!(done.installed.iter().all(|f| f.starts_with("Data/patch-") && !f.ends_with("-T.MPQ") && !f.ends_with("-Z.MPQ")));
        let man = mods::load(&c.path);
        assert_eq!(man.get("look:water").unwrap().source, "look");
        assert_eq!(state(&c, &app, &cat).on, vec!["look:water".to_string()]);
        // the author changed the file: nothing installed
        let mut changed = p.clone();
        changed.install.as_mut().unwrap().sha256 = Some("ab".repeat(32));
        let e = install_pack_from(&c.path, Build::V335a, &changed, changed.install.as_ref().unwrap(), &scratch).unwrap_err();
        assert!(e.contains("does not match"), "{e}");
        // the real address checks refuse a local address
        assert!(install_pack(&c.path, Build::V335a, &p, &scratch).is_err());
        // an update is a new release tag
        let m = man.get("look:water").unwrap().clone();
        assert_eq!(update_for(&m, &p), None);
        let mut newer = p.clone();
        newer.install.as_mut().unwrap().rev = "v2".into();
        assert_eq!(update_for(&m, &newer), Some("v2".into()));
        // back to the original look takes it out, files and all
        let r = reset(&c, &app).unwrap();
        assert_eq!(r.removed, 1);
        assert!(!c.path.join(&done.installed[0]).exists() && mods::load(&c.path).get("look:water").is_none());
    }

    #[test]
    fn a_ui_look_installs_its_addons_and_takes_out_only_those_it_put_in() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let c = client(&d.path().join("game"));
        let lk = look("look:frames", "ui-look", "", &["up:ruf", "up:dominos"], None);
        let mut cat = vec![lk.clone(), addon("up:ruf"), addon("up:dominos")];
        // the player had Dominos already
        fake_addon(&c.path, Build::V335a, &addon("up:dominos")).unwrap();
        let done = apply(&c, &app, &lk, &cat, &no_pack, &fake_addon).unwrap();
        assert_eq!(done.how, "addons");
        assert_eq!(done.installed, vec!["up:ruf".to_string()], "Dominos stays the player's");
        assert!(c.path.join("Interface/AddOns/Ruf/Ruf.toc").exists());
        assert_eq!(state(&c, &app, &cat).on, vec!["look:frames".to_string()]);
        assert_eq!(remove(&c, &app, "look:frames").unwrap(), 1);
        let man = mods::load(&c.path);
        assert!(man.get("up:ruf").is_none() && man.get("up:dominos").is_some(), "only what the look put in comes out");
        assert!(state(&c, &app, &cat).on.is_empty());
        // all or nothing: the second addon fails, the first comes out again
        mods::uninstall(&c.path, "up:dominos").unwrap();
        let fail_second = |cl: &Path, b: Build, a: &Project| if a.id == "up:dominos" { Err("broken".into()) } else { fake_addon(cl, b, a) };
        let e = apply(&c, &app, &lk, &cat, &no_pack, &fail_second).unwrap_err();
        assert!(e.contains("nothing was changed"), "{e}");
        assert!(mods::load(&c.path).get("up:ruf").is_none());
        // an addon that is not reviewed: a link, nothing installs
        cat.retain(|p| p.id != "up:dominos");
        assert!(apply(&c, &app, &lk, &cat, &no_pack, &fake_addon).unwrap_err().contains("author's page"));
        // reset takes the look's addons out too
        cat.push(addon("up:dominos"));
        apply(&c, &app, &lk, &cat, &no_pack, &fake_addon).unwrap();
        let r = reset(&c, &app).unwrap();
        assert_eq!(r.removed, 2, "{r:?}");
        assert!(mods::load(&c.path).mods.is_empty());
        assert!(load_applied(&app).clients.is_empty());
    }
}
