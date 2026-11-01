//! A small MPQ archive writer (format version 0, the one the 1.12 to 3.3.5a clients read), for the hybrid client
//! (docs/HYBRID-CLIENT.md): Leyline packs **its own** files, and the Forever UI files already on the player's PC, into
//! one patch archive in the player's own game folder. It never reads or copies game data.
//!
//! What it writes, and nothing more:
//! * the 32-byte header, then each file's data, then the hash table and the block table (both encrypted, as the
//!   format requires);
//! * every non-empty file in 4 KB sectors (sector size shift 3) with a sector offset table, each sector deflated
//!   (compression byte 0x02, zlib) when that is smaller, else stored as it is; no file encryption, no single-unit
//!   files, no sector CRCs, no `(attributes)`;
//! * a `(listfile)` naming every file, so the archive opens in the usual MPQ tools for checking.
//!
//! Names are stored as given, with `/` turned into `\`; lookups are without regard to case, as in the client.
//! `read` is a reader for exactly this subset, used by the tests and by the launcher to see whether an archive in a
//! slot is one it wrote.

use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;

pub const SECTOR_SHIFT: u16 = 3;
pub const SECTOR_SIZE: usize = 512 << SECTOR_SHIFT;
const FLAG_EXISTS: u32 = 0x8000_0000;
const FLAG_COMPRESS: u32 = 0x0000_0200;
const HEADER_SIZE: u32 = 32;
const LISTFILE: &str = "(listfile)";

fn crypt_table() -> &'static [u32; 0x500] {
    static T: std::sync::OnceLock<[u32; 0x500]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 0x500];
        let mut seed: u32 = 0x0010_0001;
        for i1 in 0..0x100 {
            let mut i2 = i1;
            for _ in 0..5 {
                seed = (seed.wrapping_mul(125).wrapping_add(3)) % 0x2A_AAAB;
                let hi = (seed & 0xFFFF) << 16;
                seed = (seed.wrapping_mul(125).wrapping_add(3)) % 0x2A_AAAB;
                t[i2] = hi | (seed & 0xFFFF);
                i2 += 0x100;
            }
        }
        t
    })
}

/// The format's string hash: 0 table index, 1 and 2 the two name checks, 3 the encryption key.
pub fn hash(name: &str, kind: u32) -> u32 {
    let t = crypt_table();
    let (mut s1, mut s2): (u32, u32) = (0x7FED_7FED, 0xEEEE_EEEE);
    for b in name.bytes() {
        let c = if b == b'/' { b'\\' } else { b.to_ascii_uppercase() } as u32;
        s1 = t[(kind * 0x100 + c) as usize] ^ s1.wrapping_add(s2);
        s2 = c.wrapping_add(s1).wrapping_add(s2).wrapping_add(s2 << 5).wrapping_add(3);
    }
    s1
}

fn encrypt(words: &mut [u32], key: u32) {
    let t = crypt_table();
    let (mut s1, mut s2) = (key, 0xEEEE_EEEEu32);
    for w in words.iter_mut() {
        s2 = s2.wrapping_add(t[0x400 + (s1 & 0xFF) as usize]);
        let plain = *w;
        *w = plain ^ s1.wrapping_add(s2);
        s1 = ((!s1) << 0x15).wrapping_add(0x1111_1111) | (s1 >> 0x0B);
        s2 = plain.wrapping_add(s2).wrapping_add(s2 << 5).wrapping_add(3);
    }
}

fn decrypt(words: &mut [u32], key: u32) {
    let t = crypt_table();
    let (mut s1, mut s2) = (key, 0xEEEE_EEEEu32);
    for w in words.iter_mut() {
        s2 = s2.wrapping_add(t[0x400 + (s1 & 0xFF) as usize]);
        let plain = *w ^ s1.wrapping_add(s2);
        *w = plain;
        s1 = ((!s1) << 0x15).wrapping_add(0x1111_1111) | (s1 >> 0x0B);
        s2 = plain.wrapping_add(s2).wrapping_add(s2 << 5).wrapping_add(3);
    }
}

/// A name the archive may hold: a relative game path, printable ASCII, no `..` part, at most 200 bytes.
pub fn name_ok(name: &str) -> bool {
    let n = name.replace('/', "\\");
    !n.is_empty()
        && n.len() <= 200
        && n.bytes().all(|b| (0x20..0x7f).contains(&b) && b != b':' && b != b'"')
        && !n.starts_with('\\')
        && n.split('\\').all(|p| !p.is_empty() && p != "." && p != "..")
}

fn pack(data: &[u8]) -> Vec<u8> {
    let n = data.len().div_ceil(SECTOR_SIZE);
    let mut table: Vec<u32> = Vec::with_capacity(n + 1);
    let mut body: Vec<u8> = Vec::new();
    let start = 4 * (n as u32 + 1);
    for chunk in data.chunks(SECTOR_SIZE) {
        table.push(start + body.len() as u32);
        let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
        z.write_all(chunk).expect("deflate into memory");
        let z = z.finish().expect("deflate into memory");
        if 1 + z.len() < chunk.len() {
            body.push(0x02);
            body.extend_from_slice(&z);
        } else {
            body.extend_from_slice(chunk);
        }
    }
    table.push(start + body.len() as u32);
    let mut out: Vec<u8> = table.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend_from_slice(&body);
    out
}

/// The archive bytes for `files` (name, contents), in the order given, plus a `(listfile)`. Refuses a bad or
/// repeated name (without regard to case), or more than 4096 files.
pub fn build(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut seen = std::collections::BTreeSet::new();
    for (n, _) in files {
        if !name_ok(n) || n.eq_ignore_ascii_case(LISTFILE) {
            return Err(format!("not a name an archive can hold: {n:?}"));
        }
        if !seen.insert(n.replace('/', "\\").to_ascii_uppercase()) {
            return Err(format!("the same file twice: {n}"));
        }
    }
    if files.len() >= 4096 {
        return Err("too many files for one archive".into());
    }
    let mut all: Vec<(String, &[u8])> = files.iter().map(|(n, d)| (n.replace('/', "\\"), d.as_slice())).collect();
    let listing: String = all.iter().map(|(n, _)| format!("{n}\r\n")).collect();
    all.push((LISTFILE.to_string(), listing.as_bytes()));

    let mut out = vec![0u8; HEADER_SIZE as usize];
    let mut blocks: Vec<[u32; 4]> = Vec::new();
    for (_, d) in &all {
        let pos = out.len() as u32;
        if d.is_empty() {
            blocks.push([pos, 0, 0, FLAG_EXISTS]);
            continue;
        }
        let p = pack(d);
        out.extend_from_slice(&p);
        blocks.push([pos, p.len() as u32, d.len() as u32, FLAG_EXISTS | FLAG_COMPRESS]);
    }
    let size = (all.len() * 2).next_power_of_two().max(16);
    let mut table = vec![[0xFFFF_FFFFu32; 4]; size];
    for (i, (n, _)) in all.iter().enumerate() {
        let mut at = (hash(n, 0) as usize) & (size - 1);
        while table[at][3] != 0xFFFF_FFFF {
            at = (at + 1) & (size - 1);
        }
        // name A, name B, locale 0 (neutral) and platform 0, block index
        table[at] = [hash(n, 1), hash(n, 2), 0, i as u32];
    }
    let mut hw: Vec<u32> = table.iter().flat_map(|e| e.iter().copied()).collect();
    encrypt(&mut hw, hash("(hash table)", 3));
    let mut bw: Vec<u32> = blocks.iter().flat_map(|e| e.iter().copied()).collect();
    encrypt(&mut bw, hash("(block table)", 3));
    let hash_pos = out.len() as u32;
    out.extend(hw.iter().flat_map(|v| v.to_le_bytes()));
    let block_pos = out.len() as u32;
    out.extend(bw.iter().flat_map(|v| v.to_le_bytes()));
    let total = out.len() as u32;
    let mut h = Vec::with_capacity(32);
    h.extend_from_slice(b"MPQ\x1a");
    h.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    h.extend_from_slice(&total.to_le_bytes());
    h.extend_from_slice(&0u16.to_le_bytes()); // format version 0
    h.extend_from_slice(&SECTOR_SHIFT.to_le_bytes());
    h.extend_from_slice(&hash_pos.to_le_bytes());
    h.extend_from_slice(&block_pos.to_le_bytes());
    h.extend_from_slice(&(size as u32).to_le_bytes());
    h.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
    out[..32].copy_from_slice(&h);
    Ok(out)
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// One file out of an archive of the subset `build` writes (header at offset 0, no encryption). None: not there,
/// or not readable by this reader.
pub fn read(archive: &[u8], name: &str) -> Option<Vec<u8>> {
    if archive.get(..4)? != b"MPQ\x1a" {
        return None;
    }
    let shift = u16::from_le_bytes([*archive.get(14)?, *archive.get(15)?]);
    let sector = 512usize << shift.min(16);
    let (hpos, bpos) = (u32_at(archive, 16)? as usize, u32_at(archive, 20)? as usize);
    let (hn, bn) = (u32_at(archive, 24)? as usize, u32_at(archive, 28)? as usize);
    if hn == 0 || !hn.is_power_of_two() || hn > 1 << 20 || bn > 1 << 20 {
        return None;
    }
    let words = |pos: usize, n: usize, key: &str| -> Option<Vec<u32>> {
        let mut w: Vec<u32> = (0..n).map(|i| u32_at(archive, pos + 4 * i)).collect::<Option<_>>()?;
        decrypt(&mut w, hash(key, 3));
        Some(w)
    };
    let ht = words(hpos, hn * 4, "(hash table)")?;
    let bt = words(bpos, bn * 4, "(block table)")?;
    let (a, b) = (hash(name, 1), hash(name, 2));
    let mut at = (hash(name, 0) as usize) & (hn - 1);
    for _ in 0..hn {
        let e = &ht[at * 4..at * 4 + 4];
        if e[3] == 0xFFFF_FFFF {
            return None;
        }
        if e[0] == a && e[1] == b && (e[3] as usize) < bn {
            let blk = &bt[e[3] as usize * 4..e[3] as usize * 4 + 4];
            let (pos, csize, size, flags) = (blk[0] as usize, blk[1] as usize, blk[2] as usize, blk[3]);
            if flags & FLAG_EXISTS == 0 || flags & !(FLAG_EXISTS | FLAG_COMPRESS) != 0 {
                return None;
            }
            if size == 0 {
                return Some(Vec::new());
            }
            let data = archive.get(pos..pos + csize)?;
            if flags & FLAG_COMPRESS == 0 {
                return Some(data.get(..size)?.to_vec());
            }
            let n = size.div_ceil(sector);
            let offs: Vec<usize> = (0..=n).map(|i| u32_at(data, 4 * i).map(|v| v as usize)).collect::<Option<_>>()?;
            let mut out = Vec::with_capacity(size);
            for i in 0..n {
                let s = data.get(offs[i]..offs[i + 1])?;
                let want = (size - i * sector).min(sector);
                if s.len() == want {
                    out.extend_from_slice(s);
                } else if s.first() == Some(&0x02) {
                    let mut d = flate2::read::ZlibDecoder::new(&s[1..]);
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut d, &mut buf).ok()?;
                    if buf.len() != want {
                        return None;
                    }
                    out.extend_from_slice(&buf);
                } else {
                    return None;
                }
            }
            return Some(out);
        }
        at = (at + 1) & (hn - 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_the_formats() {
        // The well-known key of the hash table and of the block table (every MPQ tool has these two constants).
        assert_eq!(hash("(hash table)", 3), 0xC3AF_3770);
        assert_eq!(hash("(block table)", 3), 0xEC83_B3A3);
        // case and slashes do not matter
        assert_eq!(hash("interface/gluexml/a.lua", 1), hash("Interface\\GlueXML\\A.LUA", 1));
    }

    #[test]
    fn round_trip_small_big_empty_and_incompressible() {
        let big: Vec<u8> = (0..20_000u32).flat_map(|i| format!("line {i}\n").into_bytes()).collect();
        let mut noise = Vec::new();
        let mut x: u32 = 12345;
        for _ in 0..9000 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            noise.push((x >> 16) as u8);
        }
        let files = vec![
            ("Interface/GlueXML/GlueXML.toc".to_string(), b"GlueParent.xml\r\n".to_vec()),
            ("Interface\\GlueXML\\Big.lua".to_string(), big.clone()),
            ("Interface\\GlueXML\\Empty.lua".to_string(), Vec::new()),
            ("Interface\\Glues\\Noise.blp".to_string(), noise.clone()),
            ("Interface\\Glues\\Exact.blp".to_string(), vec![7u8; SECTOR_SIZE * 2]),
        ];
        let a = build(&files).unwrap();
        assert_eq!(&a[..4], b"MPQ\x1a");
        assert_eq!(u32_at(&a, 8).unwrap() as usize, a.len(), "archive size in the header");
        assert_eq!(read(&a, "interface\\gluexml\\gluexml.toc").unwrap(), b"GlueParent.xml\r\n");
        assert_eq!(read(&a, "Interface/GlueXML/Big.lua").unwrap(), big);
        assert_eq!(read(&a, "Interface\\GlueXML\\Empty.lua").unwrap(), b"");
        assert_eq!(read(&a, "Interface\\Glues\\Noise.blp").unwrap(), noise);
        assert_eq!(read(&a, "Interface\\Glues\\Exact.blp").unwrap(), vec![7u8; SECTOR_SIZE * 2]);
        assert!(read(&a, "Interface\\GlueXML\\Missing.lua").is_none());
        let list = String::from_utf8(read(&a, "(listfile)").unwrap()).unwrap();
        assert_eq!(list.lines().count(), 5);
        assert!(list.contains("Interface\\GlueXML\\GlueXML.toc\r\n"), "names stored with backslashes");
        assert!(a.len() < big.len() / 3, "text is deflated");
        // the same input, the same bytes
        assert_eq!(build(&files).unwrap(), a);
    }

    #[test]
    fn bad_and_repeated_names_are_refused() {
        for bad in ["", "..\\x", "a\\..\\b", "\\abs", "c:\\x", "a\\\\b", "(listfile)", "caf\u{e9}"] {
            assert!(build(&[(bad.to_string(), b"x".to_vec())]).is_err(), "{bad:?}");
        }
        assert!(build(&[("A\\b.lua".into(), b"1".to_vec()), ("a/B.LUA".into(), b"2".to_vec())]).is_err());
        assert!(read(b"not an archive", "x").is_none());
    }
}
