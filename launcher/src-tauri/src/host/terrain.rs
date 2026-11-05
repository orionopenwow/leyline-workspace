//! Your Den's ground (docs/NOGGIT-DEN-PLAN.md 2.3): the edited map tile, rebuilt on this PC from the player's OWN game
//! files plus the Den's terrain delta (`leyline-terrain`, numbers and file names only), then put where each game reads
//! it. Nothing here downloads, uploads or ships game data.
//!
//! * **The delta**: `<dens>/yourden.terrain.json` beside the Den files, in whichever Den is in use (Den Lite's or the
//!   Docker Den's, host/yourden.rs `saves_root`). The first time, it is the white pad: 2 x 2 chunks around the room,
//!   flat and painted (`leyline_terrain::pad`). The one before the last change is kept as `yourden.terrain.prev.json`.
//! * **The player's own game**: `Data/patch-T.MPQ` (`crate::mpq::build`), written before a Den starts and taken out
//!   when the player plays anywhere else. T is above packs' L and below the hybrid client's Z (the client reads
//!   numbers, then letters; the last wins: docs/HYBRID-CLIENT.md section 1). mods.rs never hands out T. An archive in
//!   that slot Leyline did not write (no `Leyline\DenGround.txt` in it) is never touched. Beside the tile it holds
//!   [`den_extras`]: the player's own Light.dbc with the Den's sky made Elwynn Forest's (its lights pointed at
//!   Elwynn's existing LightParams; the Den stays on map 1: a map of its own would crash a guest's game that hasn't
//!   our patch, and only existing ids are used, so a later archive hiding the table can't leave one missing).
//! * **Forge's workspace**: `<app dir>/den-terrain/yourden/` (`World/Maps/<map>/<tile>.adt`, the tile to edit, and
//!   `locked-tiles.txt`, the one tile Forge may save). After Forge closes, [`import`] turns what it saved into the
//!   delta: a `.terrain.json` Forge wrote itself, else the diff of the saved tile against the player's own.
//! * **The Docker Den**: the rebuilt tile goes to `state/packs/den-terrain/server/World/Maps/...` with a pack.json,
//!   `packs/tiles.json` and the `packs/restage` flag, so its next start re-stages the server's map data from it
//!   (hosting/kit/tools/wuhost/denslice.py reads `den-terrain` before any other pack and before the game folder).
//! * **Den Lite** reads the delta's heights itself (labs/den-lite: the ground under props and the player).

use crate::host::dens;
use crate::mpq;
use leyline_terrain::adt::AlphaMode;
use leyline_terrain::delta::{self, Delta, DiffOptions, TileRef};
use leyline_terrain::{pad, wdt::Wdt};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const SLOT: &str = "patch-T.MPQ";
/// The live ground's name: `<dens>/yourden.terrain.json`, Forge's `--den` and its workspace folder.
pub const GROUND: &str = "yourden";
const MARK: &str = "Leyline\\DenGround.txt";
const MARK_TEXT: &[u8] = b"Leyline: your Den's ground, made on this PC from your own game files. Removed when you play elsewhere.\r\n";
/// Ours holds a few tiles and the player's own models (host/denmodels.rs); anything much bigger is someone else's.
const OURS_MAX: u64 = 512 * 1024 * 1024;
/// The pack folder the Docker Den's re-stage reads first.
pub const PACK_ID: &str = "den-terrain";

fn sha_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

// ------------------------------------------------------------------------------------------------ the delta file
pub fn ground_path(den_root: &Path) -> PathBuf {
    dens::dir(den_root).join(format!("{GROUND}.terrain.json"))
}

/// The Den's ground, if it has one.
pub fn load(den_root: &Path) -> Result<Option<Delta>, String> {
    let p = ground_path(den_root);
    match fs::metadata(&p) {
        Err(_) => Ok(None),
        Ok(m) if m.len() as usize > delta::MAX_BYTES => Err("Your Den's ground file is too big.".into()),
        Ok(_) => {
            let t = fs::read_to_string(&p).map_err(|e| format!("Couldn't read your Den's ground: {e}"))?;
            Delta::parse(&t).map(Some).map_err(|e| format!("Your Den's ground file isn't right: {e}"))
        }
    }
}

/// Saves the ground (the one before kept as `.prev`). Returns its sha256.
pub fn save(den_root: &Path, d: &Delta) -> Result<String, String> {
    let text = d.to_json().map_err(|e| e.0)?;
    let p = ground_path(den_root);
    if p.is_file() {
        let _ = fs::copy(&p, p.with_file_name(format!("{GROUND}.terrain.prev.json")));
    }
    crate::settings::write_atomic(&p, text.as_bytes()).map_err(|e| format!("Couldn't save your Den's ground: {e}"))?;
    Ok(sha_hex(text.as_bytes()))
}

// ------------------------------------------------------------------------------------------------ the player's files
/// The game's archives in the order it reads them (the last one wins): the base archives, the locale's, then the
/// patches: patch, patch-2, patch-3, patch-4 ... patch-S (mods' slots), then the locale's patches. Our own slots
/// (patch-T and up) are left out: the ground is always made from the player's own files.
pub fn archive_order(client: &Path) -> Vec<PathBuf> {
    let Some(data) = crate::clients::find_ci(client, "Data").filter(|d| d.is_dir()) else { return Vec::new() };
    let mut names: Vec<(String, PathBuf)> = Vec::new();
    let mut locale: Option<PathBuf> = None;
    if let Ok(rd) = fs::read_dir(&data) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let p = e.path();
            if p.is_dir() && n.len() == 4 && n.chars().all(|c| c.is_ascii_alphabetic()) && locale.is_none() {
                locale = Some(p);
            } else if p.is_file() && n.to_ascii_lowercase().ends_with(".mpq") {
                names.push((n.to_ascii_lowercase(), p));
            }
        }
    }
    let rank = |n: &str| -> Option<(u32, String)> {
        let stem = n.strip_suffix(".mpq")?;
        Some(match stem {
            "common" => (0, String::new()),
            "common-2" => (1, String::new()),
            "expansion" => (2, String::new()),
            "lichking" => (3, String::new()),
            "patch" => (20, String::new()),
            s => {
                let x = s.strip_prefix("patch-")?;
                let mut it = x.chars();
                let c = it.next()?.to_ascii_uppercase();
                if it.next().is_some() || !(c.is_ascii_digit() || ('A'..='S').contains(&c)) {
                    return None;
                }
                (21, c.to_string()) // digits sort before letters, as the client reads them
            }
        })
    };
    let mut base: Vec<(u32, String, PathBuf)> = names.into_iter().filter_map(|(n, p)| rank(&n).map(|(r, k)| (r, k, p))).collect();
    if let Some(l) = locale {
        let loc = l.file_name().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if let Ok(rd) = fs::read_dir(&l) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_ascii_lowercase();
                let r = if n == format!("locale-{loc}.mpq") {
                    Some((10, String::new()))
                } else if n == format!("expansion-locale-{loc}.mpq") {
                    Some((11, String::new()))
                } else if n == format!("lichking-locale-{loc}.mpq") {
                    Some((12, String::new()))
                } else if n == format!("patch-{loc}.mpq") {
                    Some((30, "1".into()))
                } else if n == format!("patch-{loc}-2.mpq") {
                    Some((30, "2".into()))
                } else if n == format!("patch-{loc}-3.mpq") {
                    Some((30, "3".into()))
                } else {
                    None
                };
                if let Some((r, k)) = r {
                    base.push((r, k, e.path()));
                }
            }
        }
    }
    base.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    base.into_iter().map(|(_, _, p)| p).collect()
}

/// The map's alpha form (its WDT) and the tile, both from the player's own archives.
pub fn own_tile(client: &Path, tile: &TileRef) -> Result<(Vec<u8>, AlphaMode), String> {
    let order = archive_order(client);
    if order.is_empty() {
        return Err("This game folder has no game archives in Data.".into());
    }
    let alpha = match mpq::read_own(&order, &tile.wdt_path())? {
        Some(w) => Wdt::parse(&w).map(|w| w.alpha_mode()).ok(),
        None => None,
    };
    let adt = mpq::read_own(&order, &tile.adt_path())?.ok_or("Your game folder hasn't the Den's map tile: is it the game folder Your Den plays with?")?;
    let alpha = alpha.unwrap_or_else(|| leyline_terrain::adt::Adt::parse(&adt, None).map(|a| a.alpha).unwrap_or(AlphaMode::Small));
    Ok((adt, alpha))
}

/// The Den's first ground on the player's own tile.
/// The whole tile (owner 1 Nov 20:55: "a nearly infinite plane of flat ... ground"): 533 yards a side, flat, the
/// island's water, trees and buildings gone. Since 3 Nov (owner 08:41: "a default Elwynn ground color grass texture
/// and ... a realm skybox") painted the game's own grass (`pad::PAD_TEXTURE`, a texture path the player's own game
/// has: only its name is in the tile, see [`den_texture_name`]) under the area's normal sky.
pub const FIRST_RING: u32 = 15;

pub fn first_ground(client: &Path) -> Result<Delta, String> {
    let tile = pad::den_tile();
    let (base, _) = own_tile(client, &tile)?;
    let p = delta::Pad { height: pad::DEN.2, texture: pad::PAD_TEXTURE.into(), centre: None };
    pad::make_pad(&base, tile, p, FIRST_RING).map_err(|e| e.0)
}

// ------------------------------------------------------------------------------------------------ the Den's world
/// What the Den's patch-T holds besides the tile, for the test's report (`<dens>/yourden.ground-status.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct GroundStatus {
    /// The Den's chunk is painted with the game's own grass (the name in [`GroundStatus::texture_name`]), found in
    /// the player's own archives.
    #[serde(default)]
    pub grass: bool,
    /// Chunks of the tile open to the Den (256: all).
    pub open: usize,
    /// The island's own models and buildings still on the Den's tile (0: none), and every model on it (the player's
    /// own from Forge too).
    pub island_models: usize,
    pub models: usize,
    /// The sky: [`SKY`] (Elwynn Forest's, from the player's own Light.dbc: 3 Nov 10:29), or "the area's own (why)".
    pub sky: String,
    /// The game's own texture name the ground is painted with, when the player's game has it.
    #[serde(default)]
    pub texture_name: Option<String>,
    /// The zone's name: the game's own (no table changed).
    pub area: String,
    pub area_id: u32,
    /// Files of ours that an archive the game reads after patch-T also has (it hides ours): "patch-Z.MPQ: ...".
    pub hidden_by: Vec<String>,
    /// What a ground from before got this time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whitened: Option<String>,
    /// The gliding copy's creature rows (host/creaturemodels.rs): "<n> prop models can glide" or "no glide (why)".
    #[serde(default)]
    pub creatures: String,
}

pub const STATUS: &str = "yourden.ground-status.json";
/// What the status says of the sky and the zone's name. The sky is Elwynn Forest's (owner 3 Nov 10:29: "do blue forest
/// sky not grey green"): the player's own Light.dbc, its Den lights pointed at Elwynn's LightParams
/// (`leyline_terrain::dbc::elwynn_sky`), is the one table of the game's in patch-T. The zone keeps the game's own name
/// (the white sky and the "Your Den" name of 2 Nov, LightParams / LightIntBand / AreaTable edits, are gone).
pub const SKY: &str = "the Elwynn sky";
pub const AREA: &str = "the game's own";
pub const LIGHT_TABLE: &str = "DBFilesClient\\Light.dbc";

pub fn status_path(den_root: &Path) -> PathBuf {
    dens::dir(den_root).join(STATUS)
}

/// The last Den start's [`GroundStatus`].
pub fn last_status(den_root: &Path) -> Option<GroundStatus> {
    fs::read_to_string(status_path(den_root)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

/// The game's archives after our slot (patch-U ... patch-Z: the game reads them later, so they win over patch-T).
fn after_slot(client: &Path) -> Vec<PathBuf> {
    let Ok(data) = data_dir(client) else { return Vec::new() };
    let mut v: Vec<PathBuf> = fs::read_dir(&data)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            n.len() == "patch-u.mpq".len() && n.starts_with("patch-") && n.ends_with(".mpq") && (b'u'..=b'z').contains(&n.as_bytes()[6])
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

/// Which grass name, of a game's listfile names, the Den's ground uses when the game hasn't `pad::PAD_TEXTURE` itself:
/// the Elwynn grass (a "grassbase" first, then the shortest name), else any tileset grass; never a specular map
/// (`_s.blp`) or a blend. Pure, for the test.
pub fn pick_grass<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = names
        .into_iter()
        .map(str::trim)
        .filter(|n| {
            let l = n.to_ascii_lowercase();
            l.starts_with("tileset\\") && l.ends_with(".blp") && !l.ends_with("_s.blp") && l.contains("grass") && !l.contains("blend") && !l.contains("trans")
        })
        .map(String::from)
        .collect();
    v.sort_by_key(|n| {
        let l = n.to_ascii_lowercase();
        (!l.starts_with("tileset\\elwynn\\"), !l.contains("grassbase"), n.len(), l)
    });
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}

/// The game's own grass texture the Den's ground is painted with: `pad::PAD_TEXTURE` when the player's archives have
/// it, else the best grass name their listfiles show (`pick_grass`) that reads. None: the game has neither (the tile
/// keeps `pad::PAD_TEXTURE`'s name; the status says so). The texture itself is never copied: the game reads its own.
pub fn den_texture_name(client: &Path) -> Option<String> {
    let order = archive_order(client);
    let is_blp = |n: &str| crate::mpq::read_own(&order, n).ok().flatten().is_some_and(|b| b.starts_with(b"BLP2"));
    if is_blp(pad::PAD_TEXTURE) {
        return Some(pad::PAD_TEXTURE.to_string());
    }
    let mut names: Vec<String> = Vec::new();
    for p in order.iter().rev() {
        let Ok(mut a) = crate::mpq::GameArchive::open(p) else { continue };
        let Ok(Some(list)) = a.read("(listfile)") else { continue };
        names.extend(String::from_utf8_lossy(&list).lines().map(String::from));
    }
    pick_grass(names.iter().map(String::as_str)).into_iter().take(8).find(|n| is_blp(n))
}

/// Besides the tile, what the Den's patch-T holds: the player's own Light.dbc with the Den's sky made Elwynn Forest's
/// ([`SKY`]); no texture of ours, no other table. When the table doesn't read or isn't the shape Leyline knows, it is
/// left out (the sky is then the area's own) and the status says why. The status: the grass, the island's models
/// gone, the sky, the zone the game's own. Never fails.
pub fn den_extras(client: &Path, d: &Delta, tile: &[u8]) -> (Vec<(String, Vec<u8>)>, GroundStatus) {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut st = GroundStatus { open: d.open.len(), sky: SKY.into(), area: AREA.into(), ..Default::default() };
    let centre = d.pad.as_ref().and_then(|p| p.centre).map(|c| (c[0], c[1])).unwrap_or((pad::DEN.0, pad::DEN.1));
    if let Ok(t) = leyline_terrain::adt::Adt::parse(tile, None) {
        st.models = t.doodads.len() + t.wmos.len();
        st.island_models = t.doodads.iter().map(|x| x.uid).chain(t.wmos.iter().map(|x| x.uid)).filter(|&u| u < delta::UID_BASE).count();
        st.area_id = pad::area_at(&t, &d.tile, centre);
    }
    // the den-test's variant (tools/den-check "Den test - variants.bat"): LEYLINE_DEN_SKY=0 leaves the area's own sky
    st.sky = if std::env::var("LEYLINE_DEN_SKY").is_ok_and(|v| v == "0") {
        "the area's own (the test asked)".into()
    } else {
        let order = archive_order(client);
        let light = mpq::read_own(&order, LIGHT_TABLE)
            .and_then(|b| b.ok_or_else(|| "your game has no Light.dbc".to_string()))
            .and_then(|b| leyline_terrain::dbc::Dbc::parse(&b).map_err(|e| format!("Light.dbc: {}", e.0)));
        match light.and_then(|l| leyline_terrain::dbc::elwynn_sky(&l, d.tile.map, centre.0, centre.1).map_err(|e| e.0)) {
            Ok(sky) => {
                files.push((LIGHT_TABLE.to_string(), sky.light.write()));
                SKY.into()
            }
            Err(e) => format!("the area's own ({e})"),
        }
    };
    (files, st)
}

/// The tile's texture at the Den's middle (its first layer), for the status.
fn texture_at_centre(t: &leyline_terrain::adt::Adt, d: &Delta) -> Option<String> {
    let centre = d.pad.as_ref().and_then(|p| p.centre).map(|c| (c[0], c[1])).unwrap_or((pad::DEN.0, pad::DEN.1));
    let (_, (r, c)) = leyline_terrain::locate(centre.0, centre.1)?;
    t.chunks.get((r * 16 + c) as usize)?.layers.first().and_then(|l| t.textures.get(l.texture as usize)).cloned()
}

/// patch-T for this ground: the tile (its pad painted the grass name the player's game has) and our mark; the status
/// saved beside the Den files. Returns the tile's sha256.
fn write_den_patch(client: &Path, den_root: &Path, d: &Delta, tile: Vec<u8>, whitened: Option<String>) -> Result<(String, GroundStatus), String> {
    let sha = sha_hex(&tile);
    let (extras, mut st) = den_extras(client, d, &tile);
    st.whitened = whitened;
    let found = den_texture_name(client);
    let tile = match leyline_terrain::adt::Adt::parse(&tile, None) {
        Ok(mut t) => {
            // the grass name the player's game has, where the pad says pad::PAD_TEXTURE
            let mut renamed = false;
            if let Some(n) = found.as_ref().filter(|n| !n.eq_ignore_ascii_case(pad::PAD_TEXTURE)) {
                for name in t.textures.iter_mut().filter(|x| x.eq_ignore_ascii_case(pad::PAD_TEXTURE)) {
                    *name = n.clone();
                    renamed = true;
                }
            }
            let at = texture_at_centre(&t, d);
            st.grass = found.is_some() && at.as_ref().zip(found.as_ref()).is_some_and(|(a, f)| a.eq_ignore_ascii_case(f));
            st.texture_name = at;
            if renamed { t.write() } else { tile }
        }
        Err(_) => tile,
    };
    let mut files = vec![(d.tile.adt_path(), tile)];
    files.extend(extras);
    // My models (host/denmodels.rs): the player's own model files under World\Leyline\Den\ and their own model
    // table with a row each (none dropped: nothing, and the table is left alone)
    let dens_dir = dens::dir(den_root);
    if !crate::host::denmodels::list(&dens_dir).is_empty() {
        let stock = mpq::read_own(&archive_order(client), crate::host::denmodels::TABLE).ok().flatten();
        files.extend(crate::host::denmodels::patch_files(&dens_dir, stock.as_deref()));
    }
    // the gliding copy (host/creaturemodels.rs; owner 5 Nov: props "dynamically move instead of disappearing,
    // reappearing"): the player's own creature tables with a row per prop model, from the model table the patch has
    // (with the Den's own rows) or the game's own. Never stops the Den: without them the copy is a gameobject as before.
    let order = archive_order(client);
    let godi = files.iter().find(|f| f.0 == crate::host::denmodels::TABLE).map(|f| f.1.clone()).or_else(|| mpq::read_own(&order, crate::host::denmodels::TABLE).ok().flatten());
    let creatures = match godi.as_deref().map(|g| crate::host::creaturemodels::patch_files(&order, g)) {
        Some(Ok((tables, displays))) => {
            files.extend(tables);
            st.creatures = format!("{} prop models can glide", displays.len());
            Some(displays)
        }
        Some(Err(e)) => {
            st.creatures = format!("no glide ({e})");
            None
        }
        None => {
            st.creatures = "no glide (your game has no GameObjectDisplayInfo)".into();
            None
        }
    };
    // what an archive read after ours would hide
    for p in after_slot(client) {
        let Ok(mut a) = mpq::GameArchive::open(&p) else { continue };
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let hit: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).filter(|n| a.read(n).ok().flatten().is_some()).collect();
        if !hit.is_empty() {
            st.hidden_by.push(format!("{name}: {}", hit.join(", ")));
        }
    }
    write_patch(client, &files)?;
    // Den Lite glides only what the game really has rows for: not when an archive read after ours hides our tables
    let hidden = st.hidden_by.iter().any(|h| h.contains(crate::host::creaturemodels::DISPLAY_TABLE) || h.contains(crate::host::creaturemodels::MODEL_TABLE));
    if hidden && creatures.is_some() {
        st.creatures = "no glide (a later game archive hides the creature tables)".into();
    }
    crate::host::creaturemodels::write_marker(&dens_dir, creatures.as_deref().filter(|_| !hidden));
    if let Ok(t) = serde_json::to_string_pretty(&st) {
        let _ = crate::settings::write_atomic(&status_path(den_root), t.as_bytes());
    }
    Ok((sha, st))
}

/// The ground, made the first time it is asked for.
pub fn ground_or_pad(client: &Path, den_root: &Path) -> Result<Delta, String> {
    if let Some(d) = load(den_root)? {
        return Ok(d);
    }
    let d = first_ground(client)?;
    save(den_root, &d)?;
    Ok(d)
}

/// The edited tile, rebuilt from the player's own files plus the delta.
pub fn build_tile(client: &Path, d: &Delta) -> Result<Vec<u8>, String> {
    if d.base == "blank" {
        return delta::apply(d, None, None).map_err(|e| e.0);
    }
    let (base, alpha) = own_tile(client, &d.tile)?;
    delta::apply(d, Some(&base), Some(alpha)).map_err(|e| e.0)
}

// ------------------------------------------------------------------------------------------------ patch-T
fn data_dir(client: &Path) -> Result<PathBuf, String> {
    crate::clients::find_ci(client, "Data").filter(|d| d.is_dir()).ok_or_else(|| "This game folder has no Data folder.".into())
}

/// The slot's path when it is free or ours (true: ours is there).
pub fn slot(client: &Path) -> Result<(PathBuf, bool), String> {
    let data = data_dir(client)?;
    let Some(p) = crate::clients::find_ci(&data, SLOT) else { return Ok((data.join(SLOT), false)) };
    let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(u64::MAX);
    if len <= OURS_MAX && fs::read(&p).ok().is_some_and(|b| mpq::read(&b, MARK).is_some()) {
        Ok((p, true))
    } else {
        Err(format!("Data/{SLOT} belongs to another mod, so Leyline leaves it alone (your Den's ground needs that slot)."))
    }
}

/// Writes patch-T with these tiles (archive name, bytes) and our mark.
pub fn write_patch(client: &Path, files: &[(String, Vec<u8>)]) -> Result<PathBuf, String> {
    let (p, _) = slot(client)?;
    let mut all = files.to_vec();
    all.push((MARK.to_string(), MARK_TEXT.to_vec()));
    let bytes = mpq::build(&all)?;
    crate::settings::write_atomic(&p, &bytes).map_err(|e| format!("Couldn't write Data/{SLOT} (is the game still running?): {e}"))?;
    Ok(p)
}

/// Takes our patch-T away (someone else's is never touched). Ok(true): it was there.
pub fn remove_patch(client: &Path) -> Result<bool, String> {
    match slot(client) {
        Ok((p, true)) => fs::remove_file(&p).map(|_| true).map_err(|e| format!("Couldn't take Data/{SLOT} away (is the game still running?): {e}")),
        _ => Ok(false),
    }
}

/// Before a Den starts: patch-T from the player's own files plus the Den's ground (none yet: nothing, and an old one
/// taken away). Returns the tile's sha256 when one was written.
pub fn prepare_client(client: &Path, den_root: &Path) -> Result<Option<String>, String> {
    // the first Den start makes the ground (before: only the first Open in Forge did, so a new Den stood on
    // the bare island inside a room of ice blocks)
    let mut d = ground_or_pad(client, den_root)?;
    let mut whitened = None;
    if d.pad.is_some() && d.base != "blank" {
        if let Ok((base, _)) = own_tile(client, &d.tile) {
            // a ground from before (the 4-chunk pad, maybe shaped since): opened to the whole tile, what was made kept
            if d.open.len() < 256 {
                let mut g = d.clone();
                while let Ok(n) = pad::grow(&base, &g) {
                    g = n;
                }
                if g.open.len() > d.open.len() {
                    save(den_root, &g)?;
                    d = g;
                }
            }
            // a ground from before the grass (painted our white or the island's own, its trees and buildings standing)
            if let Ok(Some((g, w))) = pad::whiten(&base, &d) {
                save(den_root, &g)?;
                d = g;
                whitened = Some(format!("{} parts painted grass, {} of the island's models taken away, {} parts put in the Den's zone, {} parts made walkable", w.repainted, w.models, w.area, w.walkable));
            }
        }
    }
    let tile = build_tile(client, &d)?;
    let (sha, _) = write_den_patch(client, den_root, &d, tile, whitened)?;
    Ok(Some(sha))
}

// ------------------------------------------------------------------------------------------------ the Docker Den
/// The rebuilt tile for the Docker Den's server: `state/packs/den-terrain/server/<tile>`, its pack.json, every
/// pack's tiles in `packs/tiles.json`, and `packs/restage` (packs.py's layout; the next start re-stages).
pub fn stage_docker(kit: &Path, d: &Delta, tile: &[u8]) -> Result<(), String> {
    let packs = kit.join("state").join("packs");
    let home = packs.join(PACK_ID);
    let rel: PathBuf = d.tile.adt_path().split('\\').collect();
    let p = home.join("server").join(rel);
    let err = |e: std::io::Error| format!("Couldn't stage your Den's ground for its server: {e}");
    fs::create_dir_all(p.parent().unwrap()).map_err(err)?;
    crate::settings::write_atomic(&p, tile).map_err(err)?;
    // the server grid (denslice.py): file <dir>_<x>_<y> is grid (gx = y, gy = x)
    let man = serde_json::json!({
        "format": "leyline-pack", "version": 1, "id": PACK_ID, "title": "Your Den's ground",
        "line": "The ground you shaped, made from your own game files.",
        "tiles": [[d.tile.map, d.tile.y, d.tile.x]], "files": 1, "bytes": tile.len(), "client_files": [],
        "sha256": sha_hex(tile),
    });
    crate::settings::write_atomic(&home.join("pack.json"), serde_json::to_string_pretty(&man).unwrap().as_bytes()).map_err(err)?;
    let mut all: Vec<Vec<u64>> = Vec::new();
    if let Ok(rd) = fs::read_dir(&packs) {
        for e in rd.flatten() {
            let pj = e.path().join("pack.json");
            let Some(v) = fs::read_to_string(&pj).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else { continue };
            if v.get("format").and_then(|f| f.as_str()) != Some("leyline-pack") {
                continue;
            }
            for t in v.get("tiles").and_then(|t| t.as_array()).into_iter().flatten() {
                let t: Option<Vec<u64>> = t.as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect());
                if let Some(t) = t.filter(|t| t.len() == 3) {
                    if !all.contains(&t) {
                        all.push(t);
                    }
                }
            }
        }
    }
    all.sort();
    crate::settings::write_atomic(&packs.join("tiles.json"), serde_json::json!({ "tiles": all }).to_string().as_bytes()).map_err(err)?;
    fs::write(packs.join("restage"), format!("{PACK_ID}\n")).map_err(err)?;
    Ok(())
}

// ------------------------------------------------------------------------------------------------ Forge's workspace
#[derive(Debug, Clone)]
pub struct Workspace {
    pub dir: PathBuf,
    pub adt: PathBuf,
    pub locked: PathBuf,
    /// The tile as it was handed to Forge (its sha256), and when.
    pub seeded: String,
    pub since: std::time::SystemTime,
    /// The ground file handed to Forge beside it, in Forge's form (its sha256): Forge keeps its base, open area and
    /// pad, and writes it again on each save.
    pub seeded_delta: String,
    /// The newest Den file, for Forge's `--den` (its props drawn as boxes, for reference).
    pub den_file: Option<PathBuf>,
}

pub fn workspace_dir(app: &Path) -> PathBuf {
    app.join("den-terrain").join(GROUND)
}

/// Gets the workspace ready: the Den's tile as it is now (own files + delta), the one tile Forge may save.
pub fn workspace(app: &Path, client: &Path, den_root: &Path) -> Result<(Workspace, Delta), String> {
    let d = ground_or_pad(client, den_root)?;
    let tile = build_tile(client, &d)?;
    let own = own_adt(client, &d)?;
    let flat = leyline_terrain::flat::to_flat(&d, &own).map_err(|e| e.0)?;
    let flat_text = serde_json::to_string_pretty(&flat).map_err(|e| e.to_string())?;
    let dir = workspace_dir(app);
    let rel: PathBuf = d.tile.adt_path().split('\\').collect();
    let adt = dir.join(rel);
    let err = |e: std::io::Error| format!("Couldn't get the editor's folder ready: {e}");
    fs::create_dir_all(adt.parent().unwrap()).map_err(err)?;
    crate::settings::write_atomic(&adt, &tile).map_err(err)?;
    // the ground's grass is the game's own texture: Forge reads it from the game's archives (nothing of ours to add)
    // the ground as Forge reads it (forge/docs/DEN-MODE.md): it keeps base, open and pad from this file
    crate::settings::write_atomic(&dir.join(format!("{GROUND}.terrain.json")), flat_text.as_bytes()).map_err(err)?;
    // the tiles Forge may save: one "x,y" a line (DEN-MODE.md `--locked-tiles <file>`)
    let locked = dir.join("locked-tiles.txt");
    fs::write(&locked, format!("# Your Den's tile: only it can be saved\n{},{}\n", d.tile.x, d.tile.y)).map_err(err)?;
    let den_file = dens::list(den_root).first().map(|f| dens::dir(den_root).join(format!("{}.den.json", f.name)));
    let ws = Workspace { dir, adt, locked, seeded: sha_hex(&tile), since: std::time::SystemTime::now(), seeded_delta: sha_hex(flat_text.as_bytes()), den_file };
    Ok((ws, d))
}

/// The player's own tile, read (the base every delta is made against).
fn own_adt(client: &Path, d: &Delta) -> Result<leyline_terrain::adt::Adt, String> {
    if d.base == "blank" {
        let (z, tex) = d.pad.as_ref().map(|p| (p.height, p.texture.clone())).unwrap_or((0.0, pad::PAD_TEXTURE.into()));
        return Ok(leyline_terrain::adt::Adt::blank(d.tile.x, d.tile.y, z, &tex));
    }
    let (b, alpha) = own_tile(client, &d.tile)?;
    leyline_terrain::adt::Adt::parse(&b, Some(alpha)).map_err(|e| format!("Your own map tile couldn't be read: {e}"))
}

/// Forge's Den-mode command line (docs/NOGGIT-DEN-PLAN.md 1.3 change 4; the flags as forge/docs/DEN-MODE.md has
/// them): the player's game folder (read only), the workspace (the only place Forge writes), the Den's map and tile,
/// the one tile it may save, the newest Den file (its props as boxes, for reference), the ground's name and the
/// Den's middle.
pub fn forge_args(game: &Path, ws: &Workspace, d: &Delta) -> Vec<String> {
    let mut v: Vec<String> = vec![
        "--game".into(),
        game.display().to_string(),
        "--project".into(),
        ws.dir.display().to_string(),
        "--map".into(),
        d.tile.map.to_string(),
        "--tile".into(),
        format!("{},{}", d.tile.x, d.tile.y),
        "--locked-tiles".into(),
        ws.locked.display().to_string(),
    ];
    if let Some(f) = &ws.den_file {
        v.extend(["--den".into(), f.display().to_string()]);
    }
    let o = d.origin.unwrap_or(leyline_terrain::flat::ORIGIN);
    v.extend(["--name".into(), GROUND.into(), "--origin".into(), format!("{},{},{}", o[0], o[1], o[2])]);
    v
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Imported {
    /// Something changed and the ground was saved.
    pub changed: bool,
    pub chunks: usize,
    pub models: bool,
    /// Chunks changed outside the Den's open area (left out).
    pub ignored: usize,
    /// Other tiles Forge saved (left out: only the Den's tile is taken).
    pub other_tiles: Vec<String>,
    /// "delta" (Forge's own .terrain.json) or "tile" (the saved tile, diffed).
    pub from: String,
    pub sha256: String,
}

fn adts_under(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            adts_under(&p, out, depth + 1);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("adt")) {
            out.push(p);
        }
    }
}

/// After Forge closes: what it saved, as the Den's ground (only inside the open area, only the Den's tile).
pub fn import(client: &Path, den_root: &Path, ws: &Workspace) -> Result<Imported, String> {
    let cur = ground_or_pad(client, den_root)?;
    let mut out = Imported::default();
    let mut all = Vec::new();
    adts_under(&ws.dir, &mut all, 0);
    out.other_tiles = all.iter().filter(|p| **p != ws.adt).filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string())).collect();
    out.other_tiles.sort();
    // Forge's own ground file, when it wrote it again this session (it starts as the one [`workspace`] handed it):
    // Forge's form (docs/terrain-delta.schema.json, forge/docs/DEN-MODE.md), or ours. Unreadable: the saved tile.
    let own_file = ws.dir.join(format!("{GROUND}.terrain.json"));
    let text = fs::read(&own_file).ok().filter(|b| sha_hex(b) != ws.seeded_delta).and_then(|b| String::from_utf8(b).ok());
    let from_file = match text {
        None => None,
        Some(t) => {
            if let Ok(mut d) = Delta::parse(&t) {
                if d.tile != cur.tile {
                    return Err("The editor saved another tile than your Den's: nothing was changed.".into());
                }
                let before = d.chunks.len();
                d.chunks.retain(|c| cur.open.contains(&c.i));
                out.ignored = before - d.chunks.len();
                d.open = cur.open.clone();
                d.pad = d.pad.or(cur.pad.clone());
                Some(d)
            } else if let Ok(f) = leyline_terrain::flat::Flat::parse(&t) {
                let own = own_adt(client, &cur)?;
                let (d, rep) = leyline_terrain::flat::to_delta(&f, &own, &cur).map_err(|e| format!("The editor's ground file couldn't be used: {e}"))?;
                out.ignored = rep.ignored.len();
                Some(d)
            } else {
                None
            }
        }
    };
    let next = if let Some(d) = from_file {
        out.from = "delta".into();
        d
    } else {
        out.from = "tile".into();
        let saved = fs::read(&ws.adt).map_err(|_| "The editor's copy of your Den's tile is gone: nothing was changed.".to_string())?;
        if sha_hex(&saved) == ws.seeded {
            return Ok(out);
        }
        let (base, alpha) = own_tile(client, &cur.tile)?;
        let mut o = DiffOptions::new(cur.tile.clone());
        o.open = Some(cur.open.clone());
        o.pad = cur.pad.clone();
        o.origin = cur.origin;
        o.base = cur.base.clone();
        o.alpha = Some(alpha);
        o.made_by = Some("forge".into());
        let (d, rep) = delta::diff(&base, &saved, &o).map_err(|e| format!("The editor's tile couldn't be read: {e}"))?;
        out.ignored = rep.ignored.len();
        d
    };
    if next == cur {
        return Ok(out);
    }
    // what the player changed this time (the delta itself is against their own tile: the pad is in it too)
    let same = |a: &delta::ChunkDelta, b: &delta::ChunkDelta| delta::ChunkDelta { xy: None, ..a.clone() } == delta::ChunkDelta { xy: None, ..b.clone() };
    out.chunks = next.chunks.iter().filter(|c| !cur.chunks.iter().any(|x| same(x, c))).count()
        + cur.chunks.iter().filter(|x| !next.chunks.iter().any(|c| c.i == x.i)).count();
    out.models = next.models != cur.models;
    out.sha256 = save(den_root, &next)?;
    out.changed = true;
    Ok(out)
}

/// After the ground changed: the player's patch-T again, and the Docker Den's server tile (when that Den is in use).
/// Den Lite reads the ground file itself. Returns lines for the player (empty: all went through).
pub fn after_change(client: &Path, den_root: &Path, docker_kit: Option<&Path>) -> Vec<String> {
    let mut notes = Vec::new();
    let d = match load(den_root) {
        Ok(Some(d)) => d,
        Ok(None) => return notes,
        Err(e) => return vec![e],
    };
    let tile = match build_tile(client, &d) {
        Ok(t) => t,
        Err(e) => return vec![e],
    };
    if let Err(e) = write_den_patch(client, den_root, &d, tile.clone(), None) {
        notes.push(e);
    }
    if let Some(k) = docker_kit {
        match stage_docker(k, &d, &tile) {
            Ok(()) => notes.push("Restart your Den to bring the new ground to its server.".into()),
            Err(e) => notes.push(e),
        }
    }
    notes
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use leyline_terrain::adt::Adt;

    /// A game folder with only synthetic files: the Den's tile (a gentle slope) and Kalimdor's WDT in common.MPQ,
    /// a patch that changes nothing of ours, and a locale folder.
    pub fn game() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data/enUS")).unwrap();
        let mut t = Adt::blank(1, 1, 10.0, "Tileset\\Leyline\\Grass.blp");
        for c in t.chunks.iter_mut() {
            c.heights.iter_mut().enumerate().for_each(|(k, h)| *h = (k % 17) as f32 * 0.1);
        }
        t.alpha = AlphaMode::Small;
        let mut w = Wdt::empty(0);
        w.set_tile(1, 1, true).unwrap();
        let tile = pad::den_tile();
        let common = mpq::build(&[(tile.wdt_path(), w.write()), (tile.adt_path(), t.write())]).unwrap();
        fs::write(d.path().join("Data/common.MPQ"), common).unwrap();
        fs::write(d.path().join("Data/patch.MPQ"), mpq::build(&[("Interface\\x.lua".into(), b"--".to_vec())]).unwrap()).unwrap();
        fs::write(d.path().join("Data/enUS/locale-enUS.MPQ"), mpq::build(&[("x.txt".into(), b"x".to_vec())]).unwrap()).unwrap();
        d
    }

    #[test]
    fn the_archives_in_the_games_order_and_never_our_slots() {
        let d = game();
        for n in ["common-2.MPQ", "lichking.MPQ", "expansion.MPQ", "patch-2.MPQ", "patch-3.MPQ", "Patch-A.MPQ", "patch-4.MPQ", "patch-T.MPQ", "patch-Z.MPQ", "notes.txt"] {
            fs::write(d.path().join("Data").join(n), b"").unwrap();
        }
        fs::write(d.path().join("Data/enUS/patch-enUS.MPQ"), b"").unwrap();
        let names: Vec<String> = archive_order(d.path()).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["common.MPQ", "common-2.MPQ", "expansion.MPQ", "lichking.MPQ", "locale-enUS.MPQ", "patch.MPQ", "patch-2.MPQ", "patch-3.MPQ", "patch-4.MPQ", "Patch-A.MPQ", "patch-enUS.MPQ"]);
    }

    #[test]
    fn the_first_ground_is_the_pad_and_patch_t_holds_exactly_the_dens_tile() {
        let g = game();
        let dens = tempfile::tempdir().unwrap();
        assert!(!ground_path(dens.path()).is_file());
        let sha = prepare_client(g.path(), dens.path()).unwrap().unwrap();
        assert!(ground_path(dens.path()).is_file(), "the first Den start makes the ground");
        let d = ground_or_pad(g.path(), dens.path()).unwrap();
        assert_eq!(d.open.len(), 256, "the whole tile");
        let a = fs::read(g.path().join("Data").join(SLOT)).unwrap();
        let tile = mpq::read(&a, &d.tile.adt_path()).unwrap();
        assert_eq!(sha_hex(&tile), sha);
        let list = String::from_utf8(mpq::read(&a, "(listfile)").unwrap()).unwrap();
        assert_eq!(list.lines().count(), 2, "the tile and our mark, nothing else: no texture, no table of the game's: {list}");
        assert!(mpq::read(&a, pad::PAD_TEXTURE).is_none() && mpq::read(&a, pad::WHITE_TEXTURE).is_none());
        let st = last_status(dens.path()).unwrap();
        assert!(st.open == 256 && st.island_models == 0);
        assert!(!st.grass, "this synthetic game has no grass texture: said");
        assert_eq!(st.texture_name.as_deref(), Some(pad::PAD_TEXTURE), "the tile names the game's own grass");
        assert!(st.sky.starts_with("the area's own (your game has no Light.dbc"), "said why: {}", st.sky);
        assert_eq!(st.area, AREA);
        let t = Adt::parse(&tile, Some(AlphaMode::Small)).unwrap();
        let ground = pad::Ground::from_delta(&d).unwrap();
        assert!((ground.height_at(pad::DEN.0, pad::DEN.1).unwrap() - pad::DEN.2).abs() < 0.01);
        assert!(t.chunks[152].abs_heights().iter().all(|h| (h - pad::DEN.2).abs() < 0.01), "the pad is flat in the tile too");
        // the same files and delta on "another PC" give the same bytes
        let g2 = game();
        assert_eq!(prepare_client(g2.path(), dens.path()).unwrap().unwrap(), sha);
        // played elsewhere: taken away; someone else's archive in the slot is left alone
        assert!(remove_patch(g.path()).unwrap());
        assert!(!remove_patch(g.path()).unwrap());
        let theirs = mpq::build(&[("x.txt".into(), b"theirs".to_vec())]).unwrap();
        fs::write(g.path().join("Data").join(SLOT), &theirs).unwrap();
        assert!(prepare_client(g.path(), dens.path()).unwrap_err().contains("another mod"));
        assert!(!remove_patch(g.path()).unwrap());
        assert_eq!(fs::read(g.path().join("Data").join(SLOT)).unwrap(), theirs);
    }

    /// A table of `fields` 4-byte fields from rows of numbers.
    fn table(fields: u32, rows: &[Vec<u32>], strings: &[u8]) -> Vec<u8> {
        leyline_terrain::dbc::Dbc {
            fields,
            record_size: fields * 4,
            records: rows
                .iter()
                .map(|r| {
                    let mut v: Vec<u8> = r.iter().flat_map(|x| x.to_le_bytes()).collect();
                    v.resize(fields as usize * 4, 0);
                    v
                })
                .collect(),
            strings: strings.to_vec(),
        }
        .write()
    }

    /// The synthetic game plus synthetic tables in patch-3 (map 1's default light with params 12, a skybox, 18
    /// colour bands; the GM island's area) and the Den's tile again with its chunks in area 876 and a tree on it.
    fn game_with_tables(light_fields: u32) -> tempfile::TempDir {
        let g = game();
        let mut light = vec![1, 1, 0, 0, 0, 0, 0, 12, 12, 12, 12, 12, 12, 12, 12];
        light.truncate(light_fields as usize);
        // Elwynn Forest's light on map 0, over Goldshire (params 40 ..= 47)
        let f = |v: f32| v.to_bits();
        let (ex, ez) = ((leyline_terrain::PLACE - leyline_terrain::dbc::ELWYNN.2) * 36.0, (leyline_terrain::PLACE - leyline_terrain::dbc::ELWYNN.1) * 36.0);
        let mut elwynn = vec![2, 0, f(ex), 0, f(ez), f(9000.0), f(20000.0), 40, 41, 42, 43, 44, 45, 46, 47];
        elwynn.truncate(light_fields as usize);
        let bands: Vec<Vec<u32>> = (0..18).map(|k| vec![12 * 18 - 17 + k, 2, 0, 1440, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x101010, 0x202020]).collect();
        let mut area = vec![0u32; 36];
        area[0] = 876;
        area[1] = 1;
        (11..27).for_each(|f| area[f] = 1);
        let mut t = Adt::blank(1, 1, 10.0, "Tileset\\Leyline\\Grass.blp");
        t.alpha = AlphaMode::Small;
        t.chunks.iter_mut().for_each(|c| c.area = 876);
        t.m2_names = vec!["World\\Leyline\\Tree.m2".into()];
        t.doodads = vec![leyline_terrain::adt::Doodad { name: 0, uid: 77, pos: [leyline_terrain::PLACE - pad::DEN.1, 13.0, leyline_terrain::PLACE - pad::DEN.0], rot: [0.0; 3], scale: 1024, flags: 0 }];
        t.recompute_refs();
        let files = vec![
            ("DBFilesClient\\Light.dbc".to_string(), table(light_fields, &[light, elwynn], &[0])),
            ("DBFilesClient\\LightParams.dbc".to_string(), table(9, &[vec![12, 0, 33]], &[0])),
            ("DBFilesClient\\LightIntBand.dbc".to_string(), table(34, &bands, &[0])),
            ("DBFilesClient\\AreaTable.dbc".to_string(), table(36, &[area], b"\0GM Island\0")),
            (pad::den_tile().adt_path(), t.write()),
        ];
        fs::write(g.path().join("Data/patch-3.MPQ"), mpq::build(&files).unwrap()).unwrap();
        g
    }

    /// The synthetic game with the game's own grass texture in its archives, at `name`.
    fn with_grass(g: &tempfile::TempDir, name: &str) {
        fs::write(g.path().join("Data/patch-2.MPQ"), mpq::build(&[(name.to_string(), pad::pad_texture())]).unwrap()).unwrap();
    }

    #[test]
    fn the_dens_patch_gives_each_prop_model_a_creature_row_and_tells_den_lite() {
        // owner 5 Nov: props "dynamically move instead of disappearing, reappearing": Den Lite's copy is a creature
        // wearing the prop's model, so patch-T carries the player's own creature tables with a row per prop model
        use crate::host::creaturemodels::{DISPLAY_TABLE, MARKER, MODEL_TABLE};
        use leyline_terrain::dbc::Dbc;
        let g = game_with_tables(15);
        let godi = table(19, &[vec![39, 1], vec![9007, 27]], b"\0World\\Generic\\Chair01.mdx\0World\\wmo\\Inn.wmo\0");
        let cdi = table(16, &[vec![1, 1]], &[0]);
        let cmd = table(28, &[vec![1, 0, 1]], b"\0Creature\\Bear\\Bear.mdx\0");
        let four = g.path().join("Data/patch-4.MPQ");
        fs::write(&four, mpq::build(&[(crate::host::denmodels::TABLE.to_string(), godi), (DISPLAY_TABLE.to_string(), cdi), (MODEL_TABLE.to_string(), cmd)]).unwrap()).unwrap();
        let dens = tempfile::tempdir().unwrap();
        prepare_client(g.path(), dens.path()).unwrap();
        let a = fs::read(g.path().join("Data").join(SLOT)).unwrap();
        let d = Dbc::parse(&mpq::read(&a, DISPLAY_TABLE).expect("the creature displays in patch-T")).unwrap();
        assert!(d.row(1).is_some() && d.row(70_039).is_some() && d.row(79_007).is_none(), "the chair's model, not the inn's (a building)");
        let m = Dbc::parse(&mpq::read(&a, MODEL_TABLE).expect("the creature models in patch-T")).unwrap();
        assert_eq!(m.string(Dbc::get(m.row(70_039).unwrap(), 2)), "World\\Generic\\Chair01.mdx");
        let marker = crate::host::dens::dir(dens.path()).join(MARKER);
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!((v["base"].as_u64(), v["displays"].as_array().map(|a| a.len())), (Some(70_000), Some(1)));
        assert!(last_status(dens.path()).unwrap().creatures.starts_with("1 prop"));
        // a game without creature tables: the patch without them, and no marker (Den Lite's copy a gameobject)
        fs::write(&four, mpq::build(&[(crate::host::denmodels::TABLE.to_string(), table(19, &[vec![39, 1]], b"\0World\\Generic\\Chair01.mdx\0"))]).unwrap()).unwrap();
        prepare_client(g.path(), dens.path()).unwrap();
        assert!(!marker.exists());
        assert!(last_status(dens.path()).unwrap().creatures.starts_with("no glide"));
    }

    #[test]
    fn the_den_is_grass_under_the_areas_own_sky_with_no_table_changed() {
        // owner 3 Nov 08:41: "a default Elwynn ground color grass texture and ... a realm skybox": the game's own grass
        // by name and its zone; 10:29 "do blue forest sky not grey green": the player's own Light.dbc with the Den's
        // light pointing at Elwynn's, the one table of the game's in patch-T
        let g = game_with_tables(15);
        with_grass(&g, pad::PAD_TEXTURE);
        let dens = tempfile::tempdir().unwrap();
        prepare_client(g.path(), dens.path()).unwrap().unwrap();
        let st = last_status(dens.path()).unwrap();
        assert_eq!((st.sky.as_str(), st.area.as_str(), st.area_id), (SKY, AREA, 876), "{st:?}");
        assert!(st.grass && st.texture_name.as_deref() == Some(pad::PAD_TEXTURE), "{st:?}");
        assert!(st.island_models == 0 && st.models == 0 && st.hidden_by.is_empty(), "{st:?}");
        let a = fs::read(g.path().join("Data").join(SLOT)).unwrap();
        let list = String::from_utf8(mpq::read(&a, "(listfile)").unwrap()).unwrap();
        let tables: Vec<&str> = list.lines().filter(|l| l.to_ascii_lowercase().starts_with("dbfilesclient")).collect();
        assert_eq!(tables, vec![LIGHT_TABLE], "Light.dbc only: {list}");
        let light = leyline_terrain::dbc::Dbc::parse(&mpq::read(&a, LIGHT_TABLE).unwrap()).unwrap();
        let den_light = light.row(1).unwrap();
        assert_eq!((7..15).map(|k| leyline_terrain::dbc::Dbc::get(den_light, k)).collect::<Vec<_>>(), (40..48).collect::<Vec<u32>>(), "map 1's sky is Elwynn's");
        assert_eq!((leyline_terrain::dbc::Dbc::get(light.row(2).unwrap(), 1), leyline_terrain::dbc::Dbc::get(light.row(2).unwrap(), 7)), (0, 40), "Elwynn's own light as it was");
        assert_eq!(light.records.len(), 2, "no rows added");
        assert!(mpq::read(&a, pad::PAD_TEXTURE).is_none(), "the grass is the game's own, never copied");
        let t = Adt::parse(&mpq::read(&a, &pad::den_tile().adt_path()).unwrap(), None).unwrap();
        assert!(t.doodads.is_empty() && t.m2_names.is_empty() && t.chunks.iter().all(|c| c.refs.is_empty() && c.area == 876), "the tree went");
        assert!(t.chunks.iter().all(|c| t.textures[c.layers[0].texture as usize] == pad::PAD_TEXTURE), "every chunk the grass");
        // after Forge: the same again
        assert!(after_change(g.path(), dens.path(), None).is_empty());
        // an archive the game reads after ours with the Den's tile: said
        fs::write(g.path().join("Data/patch-Z.MPQ"), mpq::build(&[(pad::den_tile().adt_path(), b"x".to_vec())]).unwrap()).unwrap();
        prepare_client(g.path(), dens.path()).unwrap();
        assert_eq!(last_status(dens.path()).unwrap().hidden_by, vec![format!("patch-Z.MPQ: {}", pad::den_tile().adt_path())]);
    }

    #[test]
    fn a_game_with_its_grass_under_another_name_gets_that_name() {
        let g = game_with_tables(15);
        with_grass(&g, "Tileset\\Elwynn\\ElwynnGrass01.blp");
        let dens = tempfile::tempdir().unwrap();
        prepare_client(g.path(), dens.path()).unwrap();
        let st = last_status(dens.path()).unwrap();
        assert!(st.grass && st.texture_name.as_deref() == Some("Tileset\\Elwynn\\ElwynnGrass01.blp"), "{st:?}");
        let a = fs::read(g.path().join("Data").join(SLOT)).unwrap();
        let t = Adt::parse(&mpq::read(&a, &pad::den_tile().adt_path()).unwrap(), None).unwrap();
        assert!(t.textures.iter().any(|n| n == "Tileset\\Elwynn\\ElwynnGrass01.blp") && !t.textures.iter().any(|n| n == pad::PAD_TEXTURE));
        // the ground file keeps the one name; only the tile the game gets has the player's
        assert_eq!(load(dens.path()).unwrap().unwrap().pad.unwrap().texture, pad::PAD_TEXTURE);
    }

    #[test]
    fn the_grass_name_picked_from_a_games_list() {
        let names = [
            "Tileset\\Snow\\SnowBase.blp",
            "Tileset\\Elwynn\\ElwynnGrassBase_s.blp",
            "Tileset\\Barrens\\BarrensGrass.blp",
            "Tileset\\Elwynn\\ElwynnGrassFlowers02.blp",
            "Tileset\\Elwynn\\ElwynnGrassBase.blp",
            "Tileset\\Elwynn\\ElwynnGrassBlend.blp",
            "World\\Elwynn\\Grass.blp",
        ];
        assert_eq!(
            pick_grass(names),
            vec!["Tileset\\Elwynn\\ElwynnGrassBase.blp", "Tileset\\Elwynn\\ElwynnGrassFlowers02.blp", "Tileset\\Barrens\\BarrensGrass.blp"]
        );
        assert!(pick_grass(["Tileset\\Snow\\SnowBase.blp"]).is_empty());
    }

    #[test]
    fn a_ground_from_before_is_made_grass_at_the_next_start() {
        let g = game_with_tables(15);
        with_grass(&g, pad::PAD_TEXTURE);
        let dens = tempfile::tempdir().unwrap();
        // the ground as the launcher made it before: the whole tile, painted the island's own texture, the tree kept
        let (base, _) = own_tile(g.path(), &pad::den_tile()).unwrap();
        let old = pad::make_pad(&base, pad::den_tile(), delta::Pad { height: pad::DEN.2, texture: "Tileset\\Leyline\\Grass.blp".into(), centre: None }, FIRST_RING).unwrap();
        let mut old = old;
        old.models = None; // the old pad kept the island's models
        save(dens.path(), &old).unwrap();
        prepare_client(g.path(), dens.path()).unwrap();
        let d = load(dens.path()).unwrap().unwrap();
        assert_eq!(d.pad.as_ref().unwrap().texture, pad::PAD_TEXTURE);
        let st = last_status(dens.path()).unwrap();
        assert!(st.grass && st.island_models == 0, "{st:?}");
        assert!(st.whitened.as_deref().is_some_and(|w| w.starts_with("256 parts painted grass, 1 of the island's models")), "{:?}", st.whitened);
        // the start after: nothing more to do
        prepare_client(g.path(), dens.path()).unwrap();
        assert!(last_status(dens.path()).unwrap().whitened.is_none());
        assert_eq!(load(dens.path()).unwrap().unwrap(), d);
    }

    fn hill(ws: &Workspace, chunks: &[usize], up: f32) {
        let b = fs::read(&ws.adt).unwrap();
        let mut t = Adt::parse(&b, None).unwrap();
        for &i in chunks {
            t.chunks[i].heights.iter_mut().for_each(|h| *h += up);
        }
        fs::write(&ws.adt, t.write()).unwrap();
    }

    #[test]
    fn what_forge_saves_becomes_the_ground_inside_the_open_area_only() {
        let g = game();
        let app = tempfile::tempdir().unwrap();
        let dens = tempfile::tempdir().unwrap();
        let (ws, d) = workspace(app.path(), g.path(), dens.path()).unwrap();
        assert!(ws.adt.ends_with("World/Maps/Kalimdor/Kalimdor_1_1.adt"));
        let args = forge_args(g.path(), &ws, &d);
        assert_eq!(&args[4..8], ["--map", "1", "--tile", "1,1"]);
        assert_eq!(&args[10..], ["--name", "yourden", "--origin", "16226.2,16257,13.2"], "no Den file yet: no --den");
        assert!(fs::read_to_string(&ws.locked).unwrap().ends_with("\n1,1\n"));
        let handed = leyline_terrain::flat::Flat::parse(&fs::read_to_string(ws.dir.join("yourden.terrain.json")).unwrap()).unwrap();
        assert_eq!((handed.base.as_str(), handed.open.clone()), ("own:1/1/1", d.open.clone()), "Forge gets the base and the open area");
        // closed without saving: nothing
        assert_eq!(import(g.path(), dens.path(), &ws).unwrap(), Imported { from: "tile".into(), ..Default::default() });
        // a hill on the pad (152) and one at the tile's corner (0, open too: the ground is the whole tile); another tile saved too
        hill(&ws, &[152, 0], 10.0);
        fs::write(ws.adt.with_file_name("Kalimdor_2_1.adt"), b"x").unwrap();
        let r = import(g.path(), dens.path(), &ws).unwrap();
        assert!(r.changed && r.from == "tile");
        assert_eq!((r.chunks, r.ignored), (2, 0));
        assert_eq!(r.other_tiles, vec!["Kalimdor_2_1.adt".to_string()]);
        let now = load(dens.path()).unwrap().unwrap();
        assert!(dens::dir(dens.path()).join("yourden.terrain.prev.json").is_file(), "the one before is kept");
        let ground = pad::Ground::from_delta(&now).unwrap();
        assert!((ground.height_at(pad::DEN.0, pad::DEN.1).unwrap() - (pad::DEN.2 + 10.0)).abs() < 0.01, "the room's chunk is up 10");
        // patch-T and the Docker Den follow
        let kit = tempfile::tempdir().unwrap();
        fs::create_dir_all(kit.path().join("state/packs/goldshire")).unwrap();
        fs::write(kit.path().join("state/packs/goldshire/pack.json"), r#"{"format":"leyline-pack","tiles":[[0,32,48]]}"#).unwrap();
        let notes = after_change(g.path(), dens.path(), Some(kit.path()));
        assert_eq!(notes, vec!["Restart your Den to bring the new ground to its server.".to_string()]);
        let a = fs::read(g.path().join("Data").join(SLOT)).unwrap();
        let tile = mpq::read(&a, "World\\Maps\\Kalimdor\\Kalimdor_1_1.adt").unwrap();
        let staged = fs::read(kit.path().join("state/packs/den-terrain/server/World/Maps/Kalimdor/Kalimdor_1_1.adt")).unwrap();
        assert_eq!(staged, tile, "the server and the game get the same tile");
        let tiles: serde_json::Value = serde_json::from_str(&fs::read_to_string(kit.path().join("state/packs/tiles.json")).unwrap()).unwrap();
        assert_eq!(tiles["tiles"], serde_json::json!([[0, 32, 48], [1, 1, 1]]));
        assert!(kit.path().join("state/packs/restage").is_file());
        // Forge's own ground file (its form) wins when it wrote it again: a hole in 135
        dens::add(dens.path(), "house.den.json", r#"{"format":"leyline-den","version":1,"name":"house","props":[]}"#).unwrap();
        let (ws2, d2) = workspace(app.path(), g.path(), dens.path()).unwrap();
        let args = forge_args(g.path(), &ws2, &d2);
        let at = args.iter().position(|a| a == "--den").expect("a Den file: --den");
        assert!(args[at + 1].ends_with("house.den.json"));
        let p = ws2.dir.join("yourden.terrain.json");
        let mut f = leyline_terrain::flat::Flat::parse(&fs::read_to_string(&p).unwrap()).unwrap();
        f.chunks.iter_mut().find(|c| c.i == 135).unwrap().holes = 1;
        f.made = Some(leyline_terrain::delta::Made { by: Some("forge".into()), at: Some("2026-11-01T05:00:00Z".into()) });
        fs::write(&p, serde_json::to_string(&f).unwrap()).unwrap();
        let r = import(g.path(), dens.path(), &ws2).unwrap();
        assert!(r.changed && r.from == "delta" && r.chunks == 1, "{r:?}");
        let now = load(dens.path()).unwrap().unwrap();
        assert!(now.chunks.iter().any(|c| c.i == 135 && c.holes == Some(1)));
        assert!(now.chunks.iter().any(|c| c.i == 152 && c.heights.is_some()), "the hill from before stays");
        // a file that is neither form: the saved tile is used
        let (ws3, _) = workspace(app.path(), g.path(), dens.path()).unwrap();
        fs::write(ws3.dir.join("yourden.terrain.json"), "not json").unwrap();
        hill(&ws3, &[151], 2.0);
        let r = import(g.path(), dens.path(), &ws3).unwrap();
        assert!(r.changed && r.from == "tile" && r.chunks == 1, "{r:?}");
    }
}
