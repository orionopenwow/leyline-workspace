//! Den codes in the Den (denpack.rs is the format; this is the deflate and the checks of what a code holds):
//! `.den pack` makes the code of the Den as it is (its look, props, tales, island) and keeps it as
//! `<dens>/<name>.denpack`; the launcher's Saved Dens opens a code from a paste, a `leyline://pack/` link or a file
//! (ui/denpack.js, the same format), and `.den pack open <name>` opens a `.denpack` file from the Dens folder.
//!
//! What a code holds is checked like the file it stands for (den.rs parse, tale.rs parse) before anything opens, and a
//! payload that would inflate past its kind's limit stops being read there.

use crate::denpack::{self, Enc, Kind};
use std::io::{Read, Write};
use std::path::Path;

/// The text deflated (raw RFC 1951, what a browser's CompressionStream("deflate-raw") reads too).
pub fn deflate(text: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    if e.write_all(text).is_err() {
        return Vec::new();
    }
    e.finish().unwrap_or_default()
}

/// Inflated, at most `max` bytes (None: more than that, or not deflate).
pub fn inflate(body: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(body).take(max as u64 + 1).read_to_end(&mut out).ok()?;
    (out.len() <= max).then_some(out)
}

/// The code for a text of `kind` (deflated when that is shorter).
pub fn code_of(kind: Kind, text: &str) -> String {
    let z = deflate(text.as_bytes());
    if !z.is_empty() && z.len() < text.len() {
        denpack::seal(kind, Enc::Deflate, &z, text.as_bytes())
    } else {
        denpack::seal(kind, Enc::Raw, text.as_bytes(), text.as_bytes())
    }
}

/// What a code holds, checked.
#[derive(Debug, Clone)]
pub enum Opened {
    Den(crate::den::DenDoc, String),
    Tale(crate::tale::Tale, String),
    Stamp(crate::den::DenDoc, String),
}

/// A code (or a pasted text with one in it) opened and checked.
pub fn open(code: &str) -> Result<Opened, String> {
    let s = denpack::open(code)?;
    let text = match s.enc {
        Enc::Raw => s.body.clone(),
        Enc::Deflate => inflate(&s.body, s.kind.max_text()).ok_or("That Den code holds too much, or is damaged.")?,
    };
    denpack::verify(&s, &text)?;
    let text = String::from_utf8(text).map_err(|_| "That Den code doesn't hold text.".to_string())?;
    match s.kind {
        Kind::Den => crate::den::parse(&text).map(|d| Opened::Den(d, text)),
        Kind::Stamp => {
            let d = crate::den::parse(&text)?;
            if d.props.len() > crate::den::STAMP_MAX {
                return Err(format!("A stamp holds up to {} props.", crate::den::STAMP_MAX));
            }
            Ok(Opened::Stamp(d, text))
        }
        Kind::Tale => crate::tale::parse(&text).map(|t| Opened::Tale(t, text)).map_err(|p| format!("That tale has a problem: {p}")),
    }
}

/// The Den as it is now as a code: its file without the save time (the same Den, the same code).
pub fn den_code(d: &crate::den::Den, name: &str) -> String {
    let mut doc = d.to_doc(name, 0);
    doc.saved = None;
    let text = serde_json::to_string(&doc).unwrap_or_default();
    code_of(Kind::Den, &text)
}

/// `<dens>/<name>.denpack` (the code as text): written whole or not at all.
pub fn keep(dir: &Path, name: &str, code: &str) -> Result<std::path::PathBuf, String> {
    let path = dir.join(denpack::file_name(name));
    crate::settings::write_atomic(&path, format!("{code}\n").as_bytes()).map_err(|_| "The code's file can't be written.".to_string())?;
    Ok(path)
}

/// A `.denpack` file's code (its size checked first).
pub fn read(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|_| "There's no such code file.".to_string())?;
    if meta.len() as usize > denpack::MAX_CODE + 1024 {
        return Err("That code file is too big.".into());
    }
    std::fs::read_to_string(path).map_err(|_| "That code file can't be read.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_den_a_tale_and_the_island_go_round_as_codes() {
        // the island: small enough for a link
        let island = crate::skyisland::den_json("sky-island");
        let code = code_of(Kind::Den, &island);
        assert!(code.starts_with("ley1.den.z."), "{code}");
        assert!(denpack::link(&code).is_some(), "{} letters: a link", code.len());
        match open(&code).unwrap() {
            Opened::Den(d, text) => {
                assert_eq!(text, island);
                assert_eq!(d.room.unwrap().look, "sky");
                assert!(d.extra.get("tales").is_some());
            }
            o => panic!("{o:?}"),
        }
        let tale = crate::skyisland::WELCOME_TALE;
        assert!(matches!(open(&code_of(Kind::Tale, tale)).unwrap(), Opened::Tale(t, _) if t.steps.len() == 4));
        // a bad tale inside a good code: refused with the tale's own words
        let bad = code_of(Kind::Tale, "tale X\nstep A\n");
        assert!(open(&bad).unwrap_err().contains("no goal"));
        // a deflate bomb: stops at the kind's limit
        let big = "a".repeat(9000);
        let z = deflate(big.as_bytes());
        let bomb = denpack::seal(Kind::Tale, Enc::Deflate, &z, big.as_bytes());
        assert!(open(&bomb).is_err());
        assert!(inflate(&deflate(&vec![0u8; 600_000]), 512 * 1024).is_none());
        // a Den's own code: the same Den, the same code
        let mut den = crate::den::Den::new(crate::den::Spot::default());
        let st = crate::den::one_piece(crate::den::find_objects("chair")[0]);
        let s = den.spot;
        den.stamp(&st, s.x + 2.0, s.y, s.z, 0.0);
        assert_eq!(den_code(&den, "mine"), den_code(&den, "mine"));
        assert!(matches!(open(&den_code(&den, "mine")).unwrap(), Opened::Den(d, _) if d.props.len() == 1));
    }
}
