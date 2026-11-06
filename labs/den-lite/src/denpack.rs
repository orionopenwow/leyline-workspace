//! Pass a Den around (owner 5 Nov: "super lightweight local and passing around"): a whole Den, one tale or one stamp
//! as a short code a player can paste in a chat, put in a link (`leyline://pack/<code>`) or keep as a small file
//! (`<name>.denpack`, the same text). No server: the code IS the Den.
//!
//! ```text
//! ley1.<kind>.<enc>.<payload>.<check>
//!   kind     den | tale | stamp
//!   enc      r (the text itself) | z (deflate, raw: RFC 1951)
//!   payload  base64url without padding (A-Z a-z 0-9 - _)
//!   check    crc32 (IEEE) of the decoded text, 8 lower-case hex digits
//! ```
//!
//! What the code decodes to is checked like any file of its kind (den.rs `parse`, tale.rs `parse`) before anything
//! opens: a code never carries anything that runs, and a code too big or a payload that inflates past its kind's
//! limit is refused before it is read whole. This module is std only (the base64url, the check, the envelope); the
//! deflate is the crate's (packs.rs), so `rustc --test src/denpack.rs` checks it anywhere.

pub const MAGIC: &str = "ley1";
/// A code, at most (a file may hold one this big: 96 KB of letters for a ~70 KB deflated Den).
pub const MAX_CODE: usize = 96 * 1024;
/// The longest code a link carries (the launcher's deeplink.rs takes a link of up to 700 letters).
pub const LINK_MAX: usize = 680;
pub const LINK_PREFIX: &str = "leyline://pack/";
pub const FILE_EXT: &str = "denpack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Den,
    Tale,
    Stamp,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Den => "den",
            Kind::Tale => "tale",
            Kind::Stamp => "stamp",
        }
    }
    pub fn of(w: &str) -> Option<Kind> {
        match w {
            "den" => Some(Kind::Den),
            "tale" => Some(Kind::Tale),
            "stamp" => Some(Kind::Stamp),
            _ => None,
        }
    }
    /// The decoded text of this kind, at most (den.rs MAX_BYTES, tale.rs MAX_BYTES, a stamp's 60 props).
    pub fn max_text(self) -> usize {
        match self {
            Kind::Den => 512 * 1024,
            Kind::Tale => 8 * 1024,
            Kind::Stamp => 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enc {
    Raw,
    Deflate,
}

/// A code taken apart: its kind, how the payload is packed, the payload's bytes (deflated for [`Enc::Deflate`]) and
/// the check of the decoded text.
#[derive(Debug, Clone, PartialEq)]
pub struct Sealed {
    pub kind: Kind,
    pub enc: Enc,
    pub body: Vec<u8>,
    pub check: u32,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        let k = chunk.len() + 1;
        for i in 0..k {
            out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    };
    let b = s.as_bytes();
    if b.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() * 3 / 4);
    for chunk in b.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    // the unused low bits of a short last group must be zero (one text, one code)
    if b64_encode(&out) != s {
        return None;
    }
    Some(out)
}

/// CRC-32 (IEEE 802.3, the zip one).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// The code for a body (already deflated for [`Enc::Deflate`]) whose decoded text is `text`.
pub fn seal(kind: Kind, enc: Enc, body: &[u8], text: &[u8]) -> String {
    format!("{MAGIC}.{}.{}.{}.{:08x}", kind.word(), if enc == Enc::Raw { "r" } else { "z" }, b64_encode(body), crc32(text))
}

/// A code taken apart and its envelope checked (the text's check comes after it is decoded: [`verify`]). Spaces and
/// line breaks a chat put in are ignored; a link (`leyline://pack/<code>`) is a code too.
pub fn open(code: &str) -> Result<Sealed, String> {
    let code = find(code).ok_or("That isn't a Den code (they start with ley1.).")?;
    if code.len() > MAX_CODE {
        return Err(format!("That code is too long ({} KB at most).", MAX_CODE / 1024));
    }
    let parts: Vec<&str> = code.split('.').collect();
    if parts.len() != 5 || parts[0] != MAGIC {
        return Err("That Den code is cut short or from a newer Leyline.".into());
    }
    let kind = Kind::of(parts[1]).ok_or("That Den code is from a newer Leyline.")?;
    let enc = match parts[2] {
        "r" => Enc::Raw,
        "z" => Enc::Deflate,
        _ => return Err("That Den code is from a newer Leyline.".into()),
    };
    let body = b64_decode(parts[3]).ok_or("That Den code has a wrong letter in it (copied whole?).")?;
    if parts[4].len() != 8 || !parts[4].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
        return Err("That Den code's end is missing (copied whole?).".into());
    }
    let check = u32::from_str_radix(parts[4], 16).map_err(|_| "That Den code's end is wrong.")?;
    if enc == Enc::Raw && body.len() > kind.max_text() {
        return Err("That Den code holds too much.".into());
    }
    Ok(Sealed { kind, enc, body, check })
}

/// The decoded text against the code's check, and its size against the kind's.
pub fn verify(s: &Sealed, text: &[u8]) -> Result<(), String> {
    if text.len() > s.kind.max_text() {
        return Err("That Den code holds too much.".into());
    }
    if crc32(text) != s.check {
        return Err("That Den code was changed or copied wrong (its check doesn't match).".into());
    }
    if std::str::from_utf8(text).is_err() {
        return Err("That Den code doesn't hold text.".into());
    }
    Ok(())
}

/// The code in a pasted text: a bare code, a `leyline://pack/` link, or a code with spaces and line breaks in it.
pub fn find(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches('\u{feff}');
    let s = s.strip_prefix(LINK_PREFIX).unwrap_or(s);
    let start = s.find(&format!("{MAGIC}."))?;
    let joined: String = s[start..].chars().filter(|c| !c.is_whitespace()).collect();
    let end = joined.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')).unwrap_or(joined.len());
    let code = &joined[..end];
    // words a chat put right after it ("...0a1b2c3d thanks"): the code ends 8 letters into its fifth part
    let mut dots = code.match_indices('.').map(|(i, _)| i);
    match dots.nth(3) {
        Some(i) if code.len() > i + 9 => Some(code[..i + 9].to_string()),
        _ => Some(code.to_string()),
    }
}

/// The link for a code, when it is short enough for one.
pub fn link(code: &str) -> Option<String> {
    (code.len() <= LINK_MAX).then(|| format!("{LINK_PREFIX}{code}"))
}

/// A file name for a code (`<name>.denpack`).
pub fn file_name(name: &str) -> String {
    let n: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(24).collect();
    format!("{}.{FILE_EXT}", if n.is_empty() { "den".into() } else { n })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_both_ways_and_one_text_one_code() {
        for n in 0..70 {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let e = b64_encode(&data);
            assert!(e.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
            assert_eq!(b64_decode(&e).unwrap(), data);
        }
        assert_eq!(b64_encode(b"hello"), "aGVsbG8");
        assert!(b64_decode("aGVsbG9").is_none(), "the low bits of the last letter must be zero");
        assert!(b64_decode("a").is_none() && b64_decode("ab=c").is_none() && b64_decode("ab+c").is_none());
    }

    #[test]
    fn crc_is_the_zip_one() {
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn a_code_seals_opens_and_checks() {
        let text = b"tale Hi\nstep A\n goal talk jaina\n";
        let code = seal(Kind::Tale, Enc::Raw, text, text);
        assert!(code.starts_with("ley1.tale.r."));
        let s = open(&code).unwrap();
        assert_eq!((s.kind, s.enc, s.body.as_slice()), (Kind::Tale, Enc::Raw, &text[..]));
        verify(&s, &s.body).unwrap();
        // what a chat does to it: spaces, line breaks, a link, words round it
        let messy = format!("try this: {}\n{}  thanks!", &code[..20], &code[20..]);
        assert_eq!(open(&messy).unwrap(), s);
        assert_eq!(open(&link(&code).unwrap()).unwrap(), s);
        // a changed letter: the check (or the base64) says so
        let mut b = code.clone().into_bytes();
        let i = 16;
        b[i] = if b[i] == b'A' { b'B' } else { b'A' };
        let changed = String::from_utf8(b).unwrap();
        let r = open(&changed).and_then(|s| verify(&s, &s.body));
        assert!(r.is_err(), "{changed}");
        for bad in ["", "ley1", "ley1.den.r.AAAA", "ley2.den.r.AAAA.00000000", "ley1.map.r.AAAA.00000000", "ley1.den.x.AAAA.00000000", "ley1.den.r.AAAA.0000000G", "ley1.den.r.AAAA.ABCDEF01"] {
            assert!(open(bad).is_err(), "{bad}");
        }
        let huge = format!("ley1.den.r.{}.00000000", "A".repeat(MAX_CODE));
        assert!(open(&huge).unwrap_err().contains("too long"));
        assert!(verify(&Sealed { kind: Kind::Tale, enc: Enc::Raw, body: vec![], check: 0 }, &vec![b'a'; 9000]).unwrap_err().contains("too much"));
        assert!(link(&"x".repeat(LINK_MAX + 1)).is_none());
        assert_eq!(file_name("Sky Island!"), "skyisland.denpack");
        assert_eq!(file_name("!!"), "den.denpack");
    }
}
