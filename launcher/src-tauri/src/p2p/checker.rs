//! The hard refusal list for everything Leyline downloads outside the mod installer: the Host pack, theme
//! packs, launcher updates and (when `torrent_downloads` is on) anything fetched through a torrent.
//!
//! A download is refused, whichever way it came, when it holds or is:
//! * game data: archives, textures, models, maps, databases, by file name or by magic bytes;
//! * a game client: the game's own executables or a game `Data/` folder.
//!
//! The mod installer (`mods.rs`) has its own, stricter rules for what may go into a client; this list is the
//! floor under every other download.

use std::fs;
use std::io::Read;
use std::path::Path;

const GAME_EXT: &[&str] = &[".mpq", ".blp", ".m2", ".wmo", ".adt", ".dbc", ".db2", ".skin", ".anim", ".wdt", ".wdl", ".bls", ".casc"];
const GAME_MAGIC: &[&[u8; 4]] = &[b"MPQ\x1a", b"MPQ\x1b", b"BLP1", b"BLP2", b"MD20", b"MD21", b"WDBC", b"WDB2", b"REVM"];
/// Executables and folders that only a game client has.
const CLIENT_NAMES: &[&str] = &["wow.exe", "wow-64.exe", "wowb.exe", "wowt.exe", "wowclassic.exe", "world of warcraft.app", "wow.app", "blizzard launcher.exe", "battle.net.exe", "battle.net launcher.exe"];
/// A zip may hold at most this many entries before it is refused unread.
const MAX_ENTRIES: usize = 20_000;

fn lower_base(name: &str) -> String {
    name.trim_end_matches('/').rsplit(['/', '\\']).next().unwrap_or(name).to_ascii_lowercase()
}

/// Why a path inside a package (or a file name) is refused, or None.
pub fn name_refusal(name: &str) -> Option<String> {
    let base = lower_base(name);
    if CLIENT_NAMES.contains(&base.as_str()) {
        return Some(format!("{name}: a game client file"));
    }
    if base.starts_with("wow") && base.ends_with(".exe") {
        return Some(format!("{name}: a game client file"));
    }
    let lower = name.to_ascii_lowercase().replace('\\', "/");
    if lower.split('/').any(|p| p == "_retail_" || p == "_classic_" || p == "_classic_era_") {
        return Some(format!("{name}: a game client folder"));
    }
    if let Some(e) = GAME_EXT.iter().find(|e| base.ends_with(*e)) {
        return Some(format!("{name}: game data ({e} files are never downloaded by Leyline)"));
    }
    None
}

/// Why these first bytes are refused, or None.
pub fn magic_refusal(name: &str, head: &[u8]) -> Option<String> {
    (head.len() >= 4 && GAME_MAGIC.iter().any(|m| &head[..4] == *m)).then(|| format!("{name}: game data (by its contents)"))
}

/// Check a downloaded file: a zip is checked entry by entry (names and contents), anything else by its own
/// name and first bytes. Ok(()) means nothing on the refusal list.
pub fn check_file(path: &Path, display_name: &str) -> Result<(), String> {
    if let Some(r) = name_refusal(display_name) {
        return Err(r);
    }
    let mut head = [0u8; 4];
    let n = fs::File::open(path).and_then(|mut f| f.read(&mut head)).map_err(|e| e.to_string())?;
    if let Some(r) = magic_refusal(display_name, &head[..n]) {
        return Err(r);
    }
    if n == 4 && &head == b"PK\x03\x04" {
        return check_zip(path);
    }
    if n >= 2 && head[..2] == [0x1f, 0x8b] {
        let low = display_name.to_ascii_lowercase();
        if low.ends_with(".tar.gz") || low.ends_with(".tgz") {
            return check_targz(path);
        }
    }
    Ok(())
}

/// A source tarball (`.tar.gz`, as `p2p::share` and the release log carry upstream server source): every entry's
/// name and first bytes, like a zip. Links, devices and odd entries are refused.
pub fn check_targz(path: &Path) -> Result<(), String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut r = flate2::read::GzDecoder::new(std::io::BufReader::new(f));
    let mut hdr = [0u8; 512];
    let mut long_name: Option<String> = None;
    let mut entries = 0usize;
    let mut zeros = 0;
    let octal = |b: &[u8]| -> Option<u64> {
        let t: String = b.iter().take_while(|c| **c != 0).map(|c| *c as char).collect();
        let t = t.trim();
        if t.is_empty() { Some(0) } else { u64::from_str_radix(t, 8).ok() }
    };
    loop {
        if let Err(e) = r.read_exact(&mut hdr) {
            return if zeros > 0 { Ok(()) } else { Err(format!("not a readable tarball: {e}")) };
        }
        if hdr.iter().all(|b| *b == 0) {
            zeros += 1;
            if zeros == 2 {
                return Ok(());
            }
            continue;
        }
        zeros = 0;
        entries += 1;
        if entries > 200_000 {
            return Err("the package has too many files".into());
        }
        let cstr = |b: &[u8]| String::from_utf8_lossy(&b[..b.iter().position(|c| *c == 0).unwrap_or(b.len())]).to_string();
        let size = octal(&hdr[124..136]).ok_or("not a readable tarball (size)")?;
        let kind = hdr[156];
        let mut name = cstr(&hdr[0..100]);
        if &hdr[257..262] == b"ustar" {
            let prefix = cstr(&hdr[345..500]);
            if !prefix.is_empty() {
                name = format!("{prefix}/{name}");
            }
        }
        if let Some(l) = long_name.take() {
            name = l;
        }
        let padded = size.div_ceil(512) * 512;
        let mut body_head = Vec::new();
        match kind {
            b'L' | b'x' | b'g' => {
                // a GNU long name or pax header: its text, then the next entry
                let mut text = vec![0u8; size.min(64 * 1024) as usize];
                r.read_exact(&mut text).map_err(|e| format!("not a readable tarball: {e}"))?;
                std::io::copy(&mut (&mut r).take(padded - text.len() as u64), &mut std::io::sink()).map_err(|e| e.to_string())?;
                if kind == b'L' {
                    long_name = Some(cstr(&text));
                } else if let Some(p) = String::from_utf8_lossy(&text).lines().find_map(|l| l.split_once(" path=").map(|(_, p)| p.to_string())) {
                    long_name = Some(p);
                }
                continue;
            }
            b'0' | 0 | b'5' | b'7' => {}
            b'1' | b'2' => return Err(format!("{name}: a link inside the package")),
            _ => return Err(format!("{name}: an odd entry inside the package")),
        }
        if let Some(why) = name_refusal(&name) {
            return Err(why);
        }
        if size > 0 {
            body_head.resize(size.min(4) as usize, 0);
            r.read_exact(&mut body_head).map_err(|e| format!("not a readable tarball: {e}"))?;
            if let Some(why) = magic_refusal(&name, &body_head) {
                return Err(why);
            }
        }
        std::io::copy(&mut (&mut r).take(padded - body_head.len() as u64), &mut std::io::sink()).map_err(|e| format!("not a readable tarball: {e}"))?;
    }
}

pub fn check_zip(path: &Path) -> Result<(), String> {
    let f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("not a readable zip: {e}"))?;
    if z.len() > MAX_ENTRIES {
        return Err("the package has too many files".into());
    }
    let mut data_folder_archives = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| format!("not a readable zip: {e}"))?;
        let name = e.name().to_string();
        if let Some(r) = name_refusal(&name) {
            return Err(r);
        }
        if name.to_ascii_lowercase().contains("data/") && name.to_ascii_lowercase().ends_with(".mpq") {
            data_folder_archives += 1;
        }
        if e.is_file() {
            let mut head = [0u8; 4];
            let mut got = 0;
            while got < 4 {
                match e.read(&mut head[got..]) {
                    Ok(0) => break,
                    Ok(k) => got += k,
                    Err(err) => return Err(format!("{name}: could not be read ({err})")),
                }
            }
            if let Some(r) = magic_refusal(&name, &head[..got]) {
                return Err(r);
            }
        }
    }
    if data_folder_archives > 0 {
        return Err("a game Data folder".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mods::tests::zip_of;

    #[test]
    fn names_and_magic() {
        for bad in ["Data/common.MPQ", "x/tex.BLP", "Wow.exe", "wow-64.exe", "WowCustom.exe", "World of Warcraft.app", "a/_retail_/x.txt", "Interface/model.m2"] {
            assert!(name_refusal(bad).is_some(), "{bad}");
        }
        for ok in ["leyline-host.zip", "hostpack.json", "tools/Dockerfile", "Leyline_0.3.0_x64-setup.exe", "Leyline.AppImage", "theme/bg.webp"] {
            assert!(name_refusal(ok).is_none(), "{ok}");
        }
        assert!(magic_refusal("renamed.png", b"MPQ\x1a....").is_some());
        assert!(magic_refusal("ok.png", b"\x89PNG").is_none());
    }

    /// A .tar.gz with these (name, contents), ustar headers (tests only).
    pub fn targz_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut tar = Vec::new();
        for (name, data) in files {
            let mut h = [0u8; 512];
            h[..name.len()].copy_from_slice(name.as_bytes());
            h[100..107].copy_from_slice(b"0000644");
            h[124..135].copy_from_slice(format!("{:011o}", data.len()).as_bytes());
            h[136..147].copy_from_slice(b"00000000000");
            h[156] = b'0';
            h[257..263].copy_from_slice(b"ustar\0");
            h[148..156].copy_from_slice(b"        ");
            let sum: u32 = h.iter().map(|b| *b as u32).sum();
            h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
            tar.extend_from_slice(&h);
            tar.extend_from_slice(data);
            tar.resize(tar.len().div_ceil(512) * 512, 0);
        }
        tar.extend_from_slice(&[0u8; 1024]);
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&tar).unwrap();
        gz.finish().unwrap()
    }

    #[test]
    fn source_tarballs_are_checked_inside() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("core.tar.gz");
        fs::write(&p, targz_of(&[("mangos-classic-abc/src/game/World.cpp", b"// code"), ("mangos-classic-abc/README.md", b"# hi")])).unwrap();
        assert!(check_file(&p, "mangos-classic-abc.tar.gz").is_ok());
        fs::write(&p, targz_of(&[("x/src/a.cpp", b"int a;"), ("x/data/patch.MPQ", b"junk")])).unwrap();
        assert!(check_file(&p, "x.tar.gz").unwrap_err().contains("game data"));
        fs::write(&p, targz_of(&[("x/readme.txt", b"MPQ\x1a hidden")])).unwrap();
        assert!(check_file(&p, "x.tar.gz").unwrap_err().contains("by its contents"));
        fs::write(&p, targz_of(&[("x/bin/Wow.exe", b"MZ")])).unwrap();
        assert!(check_file(&p, "x.tar.gz").unwrap_err().contains("game client"));
        fs::write(&p, &targz_of(&[("x/a.cpp", b"int a;")])[..30]).unwrap();
        assert!(check_file(&p, "x.tar.gz").is_err(), "a cut-off tarball");
    }

    #[test]
    fn zips_are_checked_inside() {
        let d = tempfile::tempdir().unwrap();
        let good = d.path().join("good.zip");
        fs::write(&good, zip_of(&[("WrathUnleashed host/leyline", b"#!/usr/bin/env bash"), ("WrathUnleashed host/tools/Dockerfile", b"FROM x")])).unwrap();
        assert!(check_file(&good, "good.zip").is_ok());
        let renamed = d.path().join("r.zip");
        fs::write(&renamed, zip_of(&[("pack/readme.txt", b"BLP2 sneaky")])).unwrap();
        assert!(check_file(&renamed, "r.zip").unwrap_err().contains("by its contents"));
        let client = d.path().join("c.zip");
        fs::write(&client, zip_of(&[("Game/Wow.exe", b"MZ")])).unwrap();
        assert!(check_file(&client, "c.zip").unwrap_err().contains("game client"));
        let raw = d.path().join("x.bin");
        fs::write(&raw, b"MPQ\x1b").unwrap();
        assert!(check_file(&raw, "x.bin").is_err());
    }
}
