//! "My models" for Your Den (owner 3 Nov: "drag and drop new 3d models"): the player's OWN model files, checked and
//! made ready for the Den's own patch. Plain bytes only (no other crate), so every rule here is unit tested.
//!
//! What is taken (nothing is converted from other formats):
//! * an `.m2` model of the game's own kind (magic `MD20`, the format number this game reads) with its `.skin` files
//!   (`<name>00.skin` ..., as many as the model says) and its `.blp` textures;
//! * or a building: a `.wmo` root (`MVER`, `MOHD`) with its group files (`<name>_000.wmo` ..., as many as the root
//!   says) and its `.blp` textures.
//!
//! Everything goes under one folder of ours, `World\Leyline\Den\<slug>\`, so no file of the game is ever replaced:
//! the model's own texture names are pointed there (an `.m2` keeps its file names in its texture list, a `.wmo` in
//! its `MOTX` block) when a dropped `.blp` has that file name; a name with no dropped texture is left as it is (the
//! player's game may have it) and reported.
//!
//! The game finds a gameobject's model through `GameObjectDisplayInfo.dbc` (field 0 the display id, 1 the model's
//! path, 12-17 its box): [`table_with`] adds one row per model to the player's own table, after every row it has.

pub const DEN_PATH: &str = "World\\Leyline\\Den";
/// Display ids of the player's own models start after this (the game's own table ends far below it).
pub const DISPLAY_BASE: u32 = 60_000;
pub const MAX_M2: usize = 16 * 1024 * 1024;
pub const MAX_SKIN: usize = 4 * 1024 * 1024;
pub const MAX_BLP: usize = 8 * 1024 * 1024;
pub const MAX_WMO: usize = 16 * 1024 * 1024;
/// One drop, all files together.
pub const MAX_TOTAL: usize = 48 * 1024 * 1024;
pub const MAX_FILES: usize = 80;
/// The `.m2` format number this game reads.
pub const M2_VERSION: u32 = 264;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    M2,
    Skin,
    Blp,
    WmoRoot,
    WmoGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    M2,
    Wmo,
}

/// A model ready for the patch: its files (archive path, bytes), the path the table names, its box.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub slug: String,
    pub kind: ModelKind,
    pub files: Vec<(String, Vec<u8>)>,
    pub model_path: String,
    pub bbox: [f32; 6],
    /// Texture names the model uses that no dropped file had (kept as they were).
    pub missing: Vec<String>,
    /// Dropped textures the model doesn't name (left out).
    pub unused: Vec<String>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(f32::from_bits)
}
fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// A dropped file's name: the last part of any path, `<stem>.<ext>`, the stem letters, digits, spaces, `_ - .`
/// (1 to 64), the extension one of ours. (stem, extension), both lower case.
pub fn clean_file_name(name: &str) -> Option<(String, String)> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let (stem, ext) = base.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    if !matches!(ext.as_str(), "m2" | "skin" | "blp" | "wmo") {
        return None;
    }
    if stem.is_empty() || stem.len() > 64 || stem.starts_with('.') || !stem.chars().all(|c| c.is_ascii_alphanumeric() || " _-.".contains(c)) {
        return None;
    }
    Some((stem.to_ascii_lowercase(), ext))
}

/// A folder name for a model: letters, digits, `_` and `-`, lower case, up to 32.
pub fn slug(s: &str) -> String {
    let v: String = s.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let v = v.trim_matches('_').chars().take(32).collect::<String>();
    if v.is_empty() { "model".into() } else { v }
}

/// What a file is, from its name and its first bytes, within its size limit.
pub fn kind_of(name: &str, b: &[u8]) -> Result<Kind, String> {
    let (_, ext) = clean_file_name(name).ok_or_else(|| format!("File {}: only .m2, .skin, .blp and .wmo files with plain names.", short(name)))?;
    let (kind, max) = match ext.as_str() {
        "m2" => (Kind::M2, MAX_M2),
        "skin" => (Kind::Skin, MAX_SKIN),
        "blp" => (Kind::Blp, MAX_BLP),
        _ => (if b.get(12..16) == Some(b"PGOM") { Kind::WmoGroup } else { Kind::WmoRoot }, MAX_WMO),
    };
    if b.len() > max {
        return Err(format!("File {}: too big ({} MB at most).", short(name), max / (1024 * 1024)));
    }
    let magic = b.get(..4).unwrap_or(&[]);
    let ok = match kind {
        Kind::M2 => magic == b"MD20",
        Kind::Skin => magic == b"SKIN",
        Kind::Blp => magic == b"BLP2",
        Kind::WmoRoot => magic == b"REVM" && b.get(12..16) == Some(b"DHOM"),
        Kind::WmoGroup => magic == b"REVM",
    };
    if !ok {
        return Err(format!("File {}: that isn't a model file of the game's own kind.", short(name)));
    }
    Ok(kind)
}

fn short(name: &str) -> String {
    name.rsplit(['/', '\\']).next().unwrap_or(name).chars().take(64).collect()
}

/// The texture file name's last part, lower case.
fn tex_base(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase()
}

// ------------------------------------------------------------------------------------------------ .m2

/// The `.m2` header's texture list (offset 0x50) as (where each entry's file name array is, the name).
pub fn m2_textures(b: &[u8]) -> Result<Vec<(usize, String)>, String> {
    let (n, ofs) = (u32_at(b, 0x50).ok_or("short")? as usize, u32_at(b, 0x54).ok_or("short")? as usize);
    if n > 512 || ofs.checked_add(n * 16).map_or(true, |e| e > b.len()) {
        return Err("The model's texture list doesn't fit in the file.".into());
    }
    let mut out = Vec::new();
    for i in 0..n {
        let e = ofs + i * 16;
        let (kind, len, at) = (u32_at(b, e).unwrap_or(1), u32_at(b, e + 8).unwrap_or(0) as usize, u32_at(b, e + 12).unwrap_or(0) as usize);
        if kind != 0 || len == 0 {
            continue; // a texture the game picks (skin, hair...): no file name
        }
        let s = b.get(at..at.saturating_add(len)).ok_or("The model's texture names don't fit in the file.")?;
        let s = &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())];
        out.push((e + 8, String::from_utf8_lossy(s).to_string()));
    }
    Ok(out)
}

/// The `.m2` with each texture name `rename` gives a new one for pointed at that name (added at the file's end).
pub fn m2_rewrite(b: &[u8], rename: impl Fn(&str) -> Option<String>) -> Result<Vec<u8>, String> {
    let mut out = b.to_vec();
    for (arr, name) in m2_textures(b)? {
        let Some(new) = rename(&name) else { continue };
        while out.len() % 16 != 0 {
            out.push(0);
        }
        let at = out.len();
        out.extend_from_slice(new.as_bytes());
        out.push(0);
        put_u32(&mut out, arr, new.len() as u32 + 1);
        put_u32(&mut out, arr + 4, at as u32);
    }
    Ok(out)
}

/// How many `.skin` files the model has (0x44).
pub fn m2_skins(b: &[u8]) -> u32 {
    u32_at(b, 0x44).unwrap_or(0)
}

/// The model's box (0xA0: min x y z, max x y z), when it is a sensible one.
pub fn m2_box(b: &[u8]) -> Option<[f32; 6]> {
    sensible((0..6).map(|i| f32_at(b, 0xA0 + 4 * i)).collect::<Option<Vec<f32>>>()?)
}

fn sensible(v: Vec<f32>) -> Option<[f32; 6]> {
    let ok = v.iter().all(|x| x.is_finite() && x.abs() < 5000.0) && v[3] >= v[0] && v[4] >= v[1] && v[5] >= v[2];
    ok.then(|| [v[0], v[1], v[2], v[3], v[4], v[5]])
}

// ------------------------------------------------------------------------------------------------ .wmo

/// A `.wmo` file's chunks: (id as read, e.g. "REVM", where its data starts, its size).
pub fn wmo_chunks(b: &[u8]) -> Result<Vec<(String, usize, usize)>, String> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= b.len() {
        let id = String::from_utf8_lossy(&b[at..at + 4]).to_string();
        let len = u32_at(b, at + 4).unwrap_or(0) as usize;
        if at + 8 + len > b.len() {
            return Err("The building file is cut short.".into());
        }
        out.push((id, at + 8, len));
        at += 8 + len;
    }
    Ok(out)
}

/// The root's group count and box (MOHD: groups at 4, the box at 36).
pub fn wmo_header(b: &[u8]) -> Option<(u32, Option<[f32; 6]>)> {
    let (_, d, len) = wmo_chunks(b).ok()?.into_iter().find(|c| c.0 == "DHOM")?;
    if len < 60 {
        return None;
    }
    let bbox = sensible((0..6).map(|i| f32_at(b, d + 36 + 4 * i)).collect::<Option<Vec<f32>>>()?);
    Some((u32_at(b, d + 4)?, bbox))
}

/// The root's texture names (MOTX), in order.
pub fn wmo_textures(b: &[u8]) -> Vec<String> {
    let Ok(ch) = wmo_chunks(b) else { return Vec::new() };
    let Some((_, d, len)) = ch.into_iter().find(|c| c.0 == "XTOM") else { return Vec::new() };
    b[d..d + len].split(|&c| c == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).to_string()).collect()
}

/// The root with its texture names (`MOTX`) renamed where `rename` says, and every material's (`MOMT`, 64 bytes
/// each: textures at 12, 24 and 36) offsets into it moved to match.
pub fn wmo_rewrite(b: &[u8], rename: impl Fn(&str) -> Option<String>) -> Result<Vec<u8>, String> {
    let ch = wmo_chunks(b)?;
    let Some(&(_, td, tlen)) = ch.iter().find(|c| c.0 == "XTOM") else { return Ok(b.to_vec()) };
    // every string in MOTX by its old offset, then the new block
    let mut map: Vec<(u32, u32)> = Vec::new();
    let mut block: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < tlen {
        if b[td + i] == 0 {
            i += 1;
            continue;
        }
        let end = b[td + i..td + tlen].iter().position(|&c| c == 0).map(|e| i + e).unwrap_or(tlen);
        let old = String::from_utf8_lossy(&b[td + i..td + end]).to_string();
        let new = rename(&old).unwrap_or(old);
        map.push((i as u32, block.len() as u32));
        block.extend_from_slice(new.as_bytes());
        block.push(0);
        while block.len() % 4 != 0 {
            block.push(0);
        }
        i = end + 1;
    }
    let mut out = Vec::with_capacity(b.len() + block.len());
    for (id, d, len) in &ch {
        let hdr = &b[d - 8..d - 4];
        if id == "XTOM" {
            out.extend_from_slice(hdr);
            out.extend_from_slice(&(block.len() as u32).to_le_bytes());
            out.extend_from_slice(&block);
        } else if id == "TMOM" {
            let mut data = b[*d..d + len].to_vec();
            for m in 0..len / 64 {
                for f in [12, 24, 36] {
                    let at = m * 64 + f;
                    let v = u32_at(&data, at).unwrap_or(0);
                    if let Some(&(_, n)) = map.iter().find(|(o, _)| *o == v) {
                        put_u32(&mut data, at, n);
                    }
                }
            }
            out.extend_from_slice(&b[d - 8..*d]);
            out.extend_from_slice(&data);
        } else {
            out.extend_from_slice(&b[d - 8..d + len]);
        }
    }
    Ok(out)
}

/// A group file's number: `<anything>_NNN.wmo`.
fn group_no(stem: &str) -> Option<u32> {
    let (_, n) = stem.rsplit_once('_')?;
    (n.len() == 3 && n.chars().all(|c| c.is_ascii_digit())).then(|| n.parse().ok()).flatten()
}

// ------------------------------------------------------------------------------------------------ one drop

/// One drop (file name, bytes) made ready: checked, renamed under `World\Leyline\Den\<slug>\`, textures pointed there.
pub fn prepare(files: &[(String, Vec<u8>)], label: Option<&str>) -> Result<Prepared, String> {
    if files.is_empty() {
        return Err("Drop a model's files: an .m2 with its .skin and .blp files, or a .wmo with its groups.".into());
    }
    if files.len() > MAX_FILES {
        return Err(format!("Up to {MAX_FILES} files at once."));
    }
    if files.iter().map(|f| f.1.len()).sum::<usize>() > MAX_TOTAL {
        return Err(format!("That's more than {} MB at once.", MAX_TOTAL / (1024 * 1024)));
    }
    let mut kinds = Vec::new();
    for (n, b) in files {
        let (stem, _) = clean_file_name(n).ok_or_else(|| format!("File {}: only .m2, .skin, .blp and .wmo files with plain names.", short(n)))?;
        kinds.push((stem, kind_of(n, b)?, b));
    }
    let primaries: Vec<_> = kinds.iter().filter(|k| matches!(k.1, Kind::M2 | Kind::WmoRoot)).collect();
    let primary = match primaries.as_slice() {
        [one] => *one,
        [] => return Err("There's no model in that: drop the .m2 (or the building's .wmo) with its files.".into()),
        _ => return Err("One model at a time: drop one .m2 (or one building .wmo) with its own files.".into()),
    };
    let slug = slug(label.filter(|l| !l.trim().is_empty()).unwrap_or(&primary.0));
    let dir = format!("{DEN_PATH}\\{slug}");
    let blps: Vec<(String, &Vec<u8>)> = kinds.iter().filter(|k| k.1 == Kind::Blp).map(|k| (format!("{}.blp", k.0), k.2)).collect();
    let rename = |old: &str| {
        let base = tex_base(old);
        blps.iter().any(|(n, _)| *n == base).then(|| format!("{dir}\\{}", slug_file(&base)))
    };
    let used = |names: &[String]| -> (Vec<String>, Vec<String>) {
        let missing: Vec<String> = names.iter().filter(|n| rename(n).is_none()).cloned().collect();
        let unused: Vec<String> = blps.iter().filter(|(b, _)| !names.iter().any(|n| tex_base(n) == *b)).map(|(b, _)| b.clone()).collect();
        (missing, unused)
    };
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let (kind, model_path, bbox, names) = if primary.1 == Kind::M2 {
        let b = primary.2;
        if u32_at(b, 4) != Some(M2_VERSION) {
            return Err("That .m2 was made for another version of the game: this game can't draw it.".into());
        }
        let names: Vec<String> = m2_textures(b)?.into_iter().map(|t| t.1).collect();
        out.push((format!("{dir}\\{slug}.m2"), m2_rewrite(b, rename)?));
        let skins = m2_skins(b).clamp(1, 4);
        for i in 0..skins {
            let want = format!("{}{i:02}", primary.0);
            let Some(s) = kinds.iter().find(|k| k.1 == Kind::Skin && k.0 == want) else {
                return Err(format!("The model needs {want}.skin too: drop it with the .m2."));
            };
            out.push((format!("{dir}\\{slug}{i:02}.skin"), s.2.clone()));
        }
        (ModelKind::M2, format!("{dir}\\{slug}.mdx"), m2_box(b), names)
    } else {
        let b = primary.2;
        let (groups, bbox) = wmo_header(b).ok_or("That building's .wmo has no header.")?;
        if groups == 0 || groups > 512 {
            return Err("That building has no groups.".into());
        }
        let names = wmo_textures(b);
        out.push((format!("{dir}\\{slug}.wmo"), wmo_rewrite(b, rename)?));
        for g in 0..groups {
            let Some(f) = kinds.iter().find(|k| k.1 == Kind::WmoGroup && group_no(&k.0) == Some(g)) else {
                return Err(format!("The building needs its group file {}_{g:03}.wmo too.", primary.0));
            };
            out.push((format!("{dir}\\{slug}_{g:03}.wmo"), f.2.clone()));
        }
        (ModelKind::Wmo, format!("{dir}\\{slug}.wmo"), bbox, names)
    };
    let (missing, unused) = used(&names);
    for (base, bytes) in &blps {
        if !unused.contains(base) {
            out.push((format!("{dir}\\{}", slug_file(base)), (*bytes).clone()));
        }
    }
    Ok(Prepared { slug, kind, files: out, model_path, bbox: bbox.unwrap_or([-0.5, -0.5, 0.0, 0.5, 0.5, 1.0]), missing, unused })
}

/// A texture's file name in our folder (letters, digits, `_ - .`).
fn slug_file(base: &str) -> String {
    base.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect()
}

// ------------------------------------------------------------------------------------------------ the table

/// The player's own `GameObjectDisplayInfo.dbc` with one row per model (display id, model path, box) after every row
/// it has. A display id the table already has, or a table of another shape, is refused.
pub fn table_with(stock: &[u8], rows: &[(u32, String, [f32; 6])]) -> Result<Vec<u8>, String> {
    if stock.len() < 20 || &stock[..4] != b"WDBC" {
        return Err("That isn't the game's model table.".into());
    }
    let (n, fields, size, sn) = (u32_at(stock, 4).unwrap_or(0) as usize, u32_at(stock, 8).unwrap_or(0), u32_at(stock, 12).unwrap_or(0) as usize, u32_at(stock, 16).unwrap_or(0) as usize);
    if fields < 2 || size != fields as usize * 4 || 20 + n * size + sn != stock.len() {
        return Err("The game's model table isn't the shape Leyline knows.".into());
    }
    let recs = &stock[20..20 + n * size];
    let max = (0..n).filter_map(|i| u32_at(recs, i * size)).max().unwrap_or(0);
    let mut strings = stock[20 + n * size..].to_vec();
    if strings.is_empty() {
        strings.push(0);
    }
    let mut add = Vec::new();
    for (id, path, bbox) in rows {
        if *id <= max {
            return Err(format!("Display {id} is the game's own already."));
        }
        let mut r = vec![0u8; size];
        put_u32(&mut r, 0, *id);
        put_u32(&mut r, 4, strings.len() as u32);
        strings.extend_from_slice(path.as_bytes());
        strings.push(0);
        if fields >= 18 {
            for (i, v) in bbox.iter().enumerate() {
                put_u32(&mut r, (12 + i) * 4, v.to_bits());
            }
        }
        add.extend(r);
    }
    let mut out = Vec::with_capacity(stock.len() + add.len() + 256);
    out.extend_from_slice(b"WDBC");
    for v in [(n + rows.len()) as u32, fields, size as u32, strings.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(recs);
    out.extend(add);
    out.extend(strings);
    Ok(out)
}

#[cfg(test)]
pub mod fake {
    /// A small `.m2` of the game's kind: `skins` skin profiles, these texture names, a box.
    pub fn m2(skins: u32, textures: &[&str]) -> Vec<u8> {
        let mut b = vec![0u8; 0x130];
        b[..4].copy_from_slice(b"MD20");
        b[4..8].copy_from_slice(&264u32.to_le_bytes());
        b[0x44..0x48].copy_from_slice(&skins.to_le_bytes());
        for (i, v) in [-1.0f32, -2.0, 0.0, 1.0, 2.0, 3.0].iter().enumerate() {
            b[0xA0 + 4 * i..0xA4 + 4 * i].copy_from_slice(&v.to_bits().to_le_bytes());
        }
        let list = b.len();
        b.resize(list + 16 * (textures.len() + 1), 0);
        b[0x50..0x54].copy_from_slice(&((textures.len() + 1) as u32).to_le_bytes());
        b[0x54..0x58].copy_from_slice(&(list as u32).to_le_bytes());
        // the first entry a game-picked texture (type 1, no name)
        b[list..list + 4].copy_from_slice(&1u32.to_le_bytes());
        for (i, t) in textures.iter().enumerate() {
            let at = b.len();
            b.extend_from_slice(t.as_bytes());
            b.push(0);
            let e = list + 16 * (i + 1);
            b[e + 8..e + 12].copy_from_slice(&(t.len() as u32 + 1).to_le_bytes());
            b[e + 12..e + 16].copy_from_slice(&(at as u32).to_le_bytes());
        }
        b
    }

    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    /// A building root: `groups` groups, these textures, one material using each.
    pub fn wmo_root(groups: u32, textures: &[&str]) -> Vec<u8> {
        let mut motx = Vec::new();
        let mut offs = Vec::new();
        for t in textures {
            offs.push(motx.len() as u32);
            motx.extend_from_slice(t.as_bytes());
            motx.push(0);
            while motx.len() % 4 != 0 {
                motx.push(0);
            }
        }
        let mut mohd = vec![0u8; 64];
        mohd[0..4].copy_from_slice(&(textures.len() as u32).to_le_bytes());
        mohd[4..8].copy_from_slice(&groups.to_le_bytes());
        for (i, v) in [-5.0f32, -5.0, 0.0, 5.0, 5.0, 8.0].iter().enumerate() {
            mohd[36 + 4 * i..40 + 4 * i].copy_from_slice(&v.to_bits().to_le_bytes());
        }
        let mut momt = Vec::new();
        for o in &offs {
            let mut m = vec![0u8; 64];
            m[12..16].copy_from_slice(&o.to_le_bytes());
            momt.extend(m);
        }
        let mut b = chunk(b"REVM", &17u32.to_le_bytes());
        b.extend(chunk(b"DHOM", &mohd));
        b.extend(chunk(b"XTOM", &motx));
        b.extend(chunk(b"TMOM", &momt));
        b.extend(chunk(b"NGOM", b"group\0\0\0"));
        b
    }

    pub fn wmo_group() -> Vec<u8> {
        let mut b = chunk(b"REVM", &17u32.to_le_bytes());
        b.extend(chunk(b"PGOM", &[0u8; 68]));
        b
    }

    pub fn blp() -> Vec<u8> {
        let mut b = b"BLP2".to_vec();
        b.extend([0u8; 64]);
        b
    }

    /// A table of `fields` fields with these ids, as the game's.
    pub fn table(fields: u32, ids: &[u32]) -> Vec<u8> {
        let mut b = b"WDBC".to_vec();
        for v in [ids.len() as u32, fields, fields * 4, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for id in ids {
            let mut r = vec![0u8; fields as usize * 4];
            r[..4].copy_from_slice(&id.to_le_bytes());
            b.extend(r);
        }
        b.push(0);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_kinds_are_checked() {
        assert_eq!(clean_file_name("C:\\x\\My Tower.M2"), Some(("my tower".into(), "m2".into())));
        assert_eq!(clean_file_name("../../evil.blp"), Some(("evil".into(), "blp".into())), "only the last part of a path");
        assert!(clean_file_name("a.exe").is_none() && clean_file_name(".m2").is_none() && clean_file_name("a$b.m2").is_none());
        assert_eq!(kind_of("a.m2", &fake::m2(1, &[])), Ok(Kind::M2));
        assert!(kind_of("a.m2", b"MZ\x90\x00 not a model").unwrap_err().contains("isn't a model file"));
        assert!(kind_of("a.blp", &vec![b'B'; MAX_BLP + 1]).unwrap_err().contains("too big"));
        assert_eq!(kind_of("a.wmo", &fake::wmo_root(1, &[])), Ok(Kind::WmoRoot));
        assert_eq!(kind_of("a_000.wmo", &fake::wmo_group()), Ok(Kind::WmoGroup));
        assert_eq!(slug("My Tower!"), "my_tower");
        assert_eq!(slug("***"), "model");
    }

    #[test]
    fn an_m2_with_its_skin_and_textures_goes_under_our_folder() {
        let m = fake::m2(1, &["Creature\\Thing\\Tower_Skin.BLP", "World\\Generic\\stone.blp"]);
        let drop = vec![
            ("Tower.m2".to_string(), m.clone()),
            ("Tower00.skin".to_string(), b"SKIN....".to_vec()),
            ("tower_skin.blp".to_string(), fake::blp()),
            ("extra.blp".to_string(), fake::blp()),
        ];
        let p = prepare(&drop, Some("Lighthouse")).unwrap();
        assert_eq!((p.slug.as_str(), p.kind), ("lighthouse", ModelKind::M2));
        assert_eq!(p.model_path, "World\\Leyline\\Den\\lighthouse\\lighthouse.mdx");
        let names: Vec<&str> = p.files.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(names, [
            "World\\Leyline\\Den\\lighthouse\\lighthouse.m2",
            "World\\Leyline\\Den\\lighthouse\\lighthouse00.skin",
            "World\\Leyline\\Den\\lighthouse\\tower_skin.blp",
        ]);
        assert!(p.files.iter().all(|f| f.0.starts_with(DEN_PATH)), "never a game file's path");
        assert_eq!(p.missing, ["World\\Generic\\stone.blp"], "the game's own texture: kept, reported");
        assert_eq!(p.unused, ["extra.blp"]);
        assert_eq!(p.bbox, [-1.0, -2.0, 0.0, 1.0, 2.0, 3.0]);
        // the model now names our texture; everything before the added names is as it was
        let t = m2_textures(&p.files[0].1).unwrap();
        assert_eq!(t[0].1, "World\\Leyline\\Den\\lighthouse\\tower_skin.blp");
        assert_eq!(t[1].1, "World\\Generic\\stone.blp");
        assert_eq!(&p.files[0].1[..0x58], &m[..0x58]);
        // what's refused
        assert!(prepare(&drop[..1], None).unwrap_err().contains("Tower00.skin") || prepare(&drop[..1], None).unwrap_err().contains("tower00.skin"));
        assert!(prepare(&[("a.blp".into(), fake::blp())], None).unwrap_err().contains("no model"));
        let two = vec![drop[0].clone(), ("b.m2".to_string(), m.clone())];
        assert!(prepare(&two, None).unwrap_err().starts_with("One model at a time"));
        let mut old = m.clone();
        old[4..8].copy_from_slice(&260u32.to_le_bytes());
        assert!(prepare(&[("a.m2".into(), old), ("a00.skin".into(), b"SKIN".to_vec())], None).unwrap_err().contains("another version"));
    }

    #[test]
    fn a_building_with_its_groups_and_textures() {
        let root = fake::wmo_root(2, &["Dungeons\\Textures\\Wall.blp", "Dungeons\\Textures\\Floor.blp"]);
        let drop = vec![
            ("Keep.wmo".to_string(), root.clone()),
            ("Keep_000.wmo".to_string(), fake::wmo_group()),
            ("Keep_001.wmo".to_string(), fake::wmo_group()),
            ("wall.blp".to_string(), fake::blp()),
        ];
        let p = prepare(&drop, None).unwrap();
        assert_eq!((p.slug.as_str(), p.kind), ("keep", ModelKind::Wmo));
        assert_eq!(p.files.iter().map(|f| f.0.rsplit('\\').next().unwrap()).collect::<Vec<_>>(), ["keep.wmo", "keep_000.wmo", "keep_001.wmo", "wall.blp"]);
        let new = &p.files[0].1;
        assert_eq!(wmo_textures(new), ["World\\Leyline\\Den\\keep\\wall.blp", "Dungeons\\Textures\\Floor.blp"]);
        // the materials point at the moved names
        let ch = wmo_chunks(new).unwrap();
        let (_, md, _) = ch.iter().find(|c| c.0 == "TMOM").unwrap().clone();
        let (_, td, _) = ch.iter().find(|c| c.0 == "XTOM").unwrap().clone();
        let at = |m: usize| u32::from_le_bytes(new[md + m * 64 + 12..md + m * 64 + 16].try_into().unwrap()) as usize;
        let name = |o: usize| String::from_utf8_lossy(&new[td + o..td + o + new[td + o..].iter().position(|&c| c == 0).unwrap()]).to_string();
        assert_eq!((name(at(0)), name(at(1))), ("World\\Leyline\\Den\\keep\\wall.blp".to_string(), "Dungeons\\Textures\\Floor.blp".to_string()));
        assert_eq!(wmo_chunks(new).unwrap().iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["REVM", "DHOM", "XTOM", "TMOM", "NGOM"]);
        assert_eq!(p.bbox, [-5.0, -5.0, 0.0, 5.0, 5.0, 8.0]);
        assert!(prepare(&drop[..2], None).unwrap_err().contains("keep_001.wmo"));
    }

    #[test]
    fn the_table_gets_one_row_per_model_after_the_games_own() {
        let stock = fake::table(19, &[1, 2, 9000]);
        let t = table_with(&stock, &[(60001, "World\\Leyline\\Den\\a\\a.mdx".into(), [0.0, 0.0, 0.0, 1.0, 1.0, 2.0])]).unwrap();
        assert_eq!(u32_at(&t, 4), Some(4));
        let row = 20 + 3 * 76;
        assert_eq!(u32_at(&t, row), Some(60001));
        let s = u32_at(&t, row + 4).unwrap() as usize;
        let strings = &t[20 + 4 * 76..];
        assert_eq!(&strings[s..s + 26], b"World\\Leyline\\Den\\a\\a.mdx\0" as &[u8]);
        assert_eq!(f32_at(&t, row + 17 * 4), Some(2.0));
        assert!(table_with(&stock, &[(9000, "x".into(), [0.0; 6])]).unwrap_err().contains("game's own"));
        assert!(table_with(b"nope", &[]).is_err());
        assert_eq!(table_with(&stock, &[]).unwrap(), stock);
    }
}
