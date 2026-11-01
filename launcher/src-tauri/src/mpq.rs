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

// ------------------------------------------------------------------------------------------------ the player's own
/// One of the player's own game archives, read only (the Den's ground: docs/NOGGIT-DEN-PLAN.md 2.3, the tile is made
/// from the player's own files). Format version 0 and 1 headers (the 1.12 to 3.3.5a clients'), found at offset 0 or
/// at any 512-byte step, or through a user-data header; the hash and block tables (and the high block table);
/// encrypted files (the file key from the name, and the adjusted key); sectors or a single unit; stored or zlib.
/// bzip2, PKWARE implode and the audio forms are refused with a sentence (game map tiles don't use them). Only the
/// tables and the asked-for file are read, never the whole archive.
pub struct GameArchive {
    f: std::fs::File,
    base: u64,
    sector: usize,
    hashes: Vec<[u32; 4]>,
    /// (position from the archive's start, packed size, size, flags)
    blocks: Vec<(u64, u32, u32, u32)>,
}

const FLAG_IMPLODE: u32 = 0x0000_0100;
const FLAG_ENCRYPTED: u32 = 0x0001_0000;
const FLAG_KEY_V2: u32 = 0x0002_0000;
const FLAG_SINGLE: u32 = 0x0100_0000;
const FLAG_DELETE: u32 = 0x0200_0000;
const FLAG_CRC: u32 = 0x0400_0000;
/// The biggest file read out of a game archive here (a map tile is under 2 MB).
pub const GAME_FILE_MAX: u32 = 64 * 1024 * 1024;

fn read_at(f: &mut std::fs::File, pos: u64, n: usize) -> Result<Vec<u8>, String> {
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b).map_err(|_| "the archive ends too early".to_string())?;
    Ok(b)
}

fn words_of(b: &[u8]) -> Vec<u32> {
    b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

impl GameArchive {
    pub fn open(path: &std::path::Path) -> Result<GameArchive, String> {
        let mut f = std::fs::File::open(path).map_err(|e| format!("Couldn't open {}: {e}", path.display()))?;
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        let mut base = None;
        let mut off = 0u64;
        while off + 32 <= len && off < 1 << 30 {
            let h = read_at(&mut f, off, 16)?;
            if &h[..4] == b"MPQ\x1a" {
                base = Some(off);
                break;
            }
            if &h[..4] == b"MPQ\x1b" {
                base = Some(off + u32::from_le_bytes([h[8], h[9], h[10], h[11]]) as u64);
                break;
            }
            off += 512;
        }
        let base = base.ok_or_else(|| format!("{} isn't a game archive.", path.display()))?;
        let h = read_at(&mut f, base, 32)?;
        let w = |at: usize| u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]]);
        let (hsize, version, shift) = (w(4), u16::from_le_bytes([h[12], h[13]]), u16::from_le_bytes([h[14], h[15]]));
        let (mut hpos, mut bpos, hn, bn) = (w(16) as u64, w(20) as u64, w(24) as usize, w(28) as usize);
        let mut hi_bt = 0u64;
        if version >= 1 && hsize >= 44 {
            let x = read_at(&mut f, base + 32, 12)?;
            hi_bt = u64::from_le_bytes(x[..8].try_into().unwrap());
            hpos |= (u16::from_le_bytes([x[8], x[9]]) as u64) << 32;
            bpos |= (u16::from_le_bytes([x[10], x[11]]) as u64) << 32;
        }
        if hn == 0 || hn > 1 << 20 || bn > 1 << 20 || shift > 16 {
            return Err(format!("{} has tables Leyline can't read.", path.display()));
        }
        let mut hw = words_of(&read_at(&mut f, base + hpos, hn * 16)?);
        decrypt(&mut hw, hash("(hash table)", 3));
        let mut bw = words_of(&read_at(&mut f, base + bpos, bn * 16)?);
        decrypt(&mut bw, hash("(block table)", 3));
        let hi: Vec<u16> = if hi_bt != 0 {
            read_at(&mut f, base + hi_bt, bn * 2)?.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
        } else {
            vec![0; bn]
        };
        Ok(GameArchive {
            f,
            base,
            sector: 512usize << shift,
            hashes: hw.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            blocks: bw.chunks_exact(4).enumerate().map(|(i, c)| (c[0] as u64 | (hi[i] as u64) << 32, c[1], c[2], c[3])).collect(),
        })
    }

    fn find(&self, name: &str) -> Option<(u64, u32, u32, u32)> {
        let n = self.hashes.len();
        let (a, b) = (hash(name, 1), hash(name, 2));
        let mut at = (hash(name, 0) as usize) % n;
        let mut best = None;
        for _ in 0..n {
            let e = self.hashes[at];
            if e[3] == 0xFFFF_FFFF {
                break;
            }
            if e[3] != 0xFFFF_FFFE && e[0] == a && e[1] == b && (e[3] as usize) < self.blocks.len() {
                let blk = self.blocks[e[3] as usize];
                if blk.3 & FLAG_EXISTS != 0 && blk.3 & FLAG_DELETE == 0 {
                    if e[2] & 0xFFFF == 0 {
                        return Some(blk);
                    }
                    best = best.or(Some(blk));
                }
            }
            at = (at + 1) % n;
        }
        best
    }

    /// One file, or None when the archive hasn't it.
    pub fn read(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let Some((pos, csize, size, flags)) = self.find(name) else { return Ok(None) };
        if size > GAME_FILE_MAX || csize > GAME_FILE_MAX.saturating_mul(2) {
            return Err(format!("{name} is too big to read here."));
        }
        if flags & FLAG_IMPLODE != 0 {
            return Err(format!("{name} is packed in an old way Leyline doesn't read."));
        }
        let (size, csize) = (size as usize, csize as usize);
        let mut key = 0;
        if flags & FLAG_ENCRYPTED != 0 {
            let base_name = name.rsplit(['\\', '/']).next().unwrap_or(name);
            key = hash(base_name, 3);
            if flags & FLAG_KEY_V2 != 0 {
                key = key.wrapping_add(pos as u32) ^ size as u32;
            }
        }
        let raw = read_at(&mut self.f, self.base + pos, csize)?;
        let unpack = |mut s: Vec<u8>, want: usize, k: Option<u32>| -> Result<Vec<u8>, String> {
            if let Some(k) = k {
                let mut w = words_of(&s);
                decrypt(&mut w, k);
                for (i, v) in w.iter().enumerate() {
                    s[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            if s.len() >= want {
                return Ok(s[..want].to_vec());
            }
            match s.first() {
                Some(0x02) => {
                    let mut d = flate2::read::ZlibDecoder::new(&s[1..]);
                    let mut out = Vec::with_capacity(want);
                    std::io::Read::read_to_end(&mut d, &mut out).map_err(|_| format!("{name} is damaged in the archive."))?;
                    if out.len() != want {
                        return Err(format!("{name} is damaged in the archive."));
                    }
                    Ok(out)
                }
                Some(m) => Err(format!("{name} is packed in a way Leyline doesn't read (0x{m:02x}).")),
                None => Err(format!("{name} is damaged in the archive.")),
            }
        };
        let enc = |k: u32| (flags & FLAG_ENCRYPTED != 0).then_some(k);
        if size == 0 {
            return Ok(Some(Vec::new()));
        }
        if flags & FLAG_COMPRESS == 0 && flags & FLAG_SINGLE == 0 && flags & FLAG_ENCRYPTED == 0 {
            return raw.get(..size).map(|s| Some(s.to_vec())).ok_or_else(|| format!("{name} is damaged in the archive."));
        }
        if flags & FLAG_SINGLE != 0 {
            return unpack(raw, size, enc(key)).map(Some);
        }
        let n = size.div_ceil(self.sector);
        let entries = n + 1 + usize::from(flags & FLAG_CRC != 0);
        let offs: Vec<usize> = if flags & FLAG_COMPRESS != 0 {
            let mut t = words_of(raw.get(..entries * 4).ok_or_else(|| format!("{name} is damaged in the archive."))?);
            if flags & FLAG_ENCRYPTED != 0 {
                decrypt(&mut t, key.wrapping_sub(1));
            }
            t.iter().take(n + 1).map(|v| *v as usize).collect()
        } else {
            (0..=n).map(|i| (i * self.sector).min(size)).collect()
        };
        let mut out = Vec::with_capacity(size);
        for i in 0..n {
            let s = raw.get(offs[i]..offs[i + 1]).ok_or_else(|| format!("{name} is damaged in the archive."))?.to_vec();
            let want = (size - i * self.sector).min(self.sector);
            out.extend(unpack(s, want, enc(key.wrapping_add(i as u32)))?);
        }
        Ok(Some(out))
    }
}

/// A file from the player's own archives, the last archive in `order` that has it winning (the game's own rule:
/// later patches over earlier ones). Archives that can't be opened are skipped.
pub fn read_own(order: &[std::path::PathBuf], name: &str) -> Result<Option<Vec<u8>>, String> {
    for p in order.iter().rev() {
        let Ok(mut a) = GameArchive::open(p) else { continue };
        if let Some(d) = a.read(name)? {
            return Ok(Some(d));
        }
    }
    Ok(None)
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

    #[test]
    fn the_game_archive_reader_reads_ours_and_encrypted_single_and_offset_ones() {
        let big: Vec<u8> = (0..20_000u32).flat_map(|i| format!("v {i}\n").into_bytes()).collect();
        let a = build(&[("World\\Maps\\Kalimdor\\Kalimdor_1_1.adt".into(), big.clone()), ("x\\y.txt".into(), b"hi".to_vec())]).unwrap();
        let d = tempfile::tempdir().unwrap();
        // a user-data header in front (as some archives have), our archive 1024 bytes in
        let mut shifted = b"MPQ\x1b".to_vec();
        shifted.extend_from_slice(&[0u8; 4]);
        shifted.extend_from_slice(&1024u32.to_le_bytes());
        shifted.resize(1024, 0);
        shifted.extend_from_slice(&a);
        let p = d.path().join("common.MPQ");
        fs_write(&p, &shifted);
        let mut g = GameArchive::open(&p).unwrap();
        assert_eq!(g.read("world/maps/kalimdor/KALIMDOR_1_1.ADT").unwrap().unwrap(), big);
        assert_eq!(g.read("x\\y.txt").unwrap().unwrap(), b"hi");
        assert!(g.read("nope").unwrap().is_none());
        // an encrypted single-unit stored file and an encrypted compressed sector file, made by hand
        let secret = b"encrypted bytes of a file, long enough to have words".to_vec();
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), Compression::best());
        z.write_all(&big).unwrap();
        let zb = z.finish().unwrap();
        let arch = hand_archive(&[("Data\\Secret.txt", secret.clone(), false), ("Data\\Big.adt", big.clone(), true)], &zb);
        let q = d.path().join("patch.MPQ");
        fs_write(&q, &arch);
        let mut g = GameArchive::open(&q).unwrap();
        assert_eq!(g.read("Data\\Secret.txt").unwrap().unwrap(), secret);
        assert_eq!(g.read("Data\\Big.adt").unwrap().unwrap(), big);
        // the order: the later archive wins
        let other = build(&[("x\\y.txt".into(), b"patched".to_vec())]).unwrap();
        let r = d.path().join("patch-2.MPQ");
        fs_write(&r, &other);
        assert_eq!(read_own(&[p.clone(), r.clone()], "x\\y.txt").unwrap().unwrap(), b"patched");
        assert_eq!(read_own(&[r, p.clone()], "x\\y.txt").unwrap().unwrap(), b"hi");
        assert!(read_own(&[d.path().join("missing.MPQ"), p], "nope").unwrap().is_none());
        assert!(GameArchive::open(&d.path().join("missing.MPQ")).is_err());
    }

    fn fs_write(p: &std::path::Path, b: &[u8]) {
        std::fs::write(p, b).unwrap();
    }

    /// An archive with (name, data, compressed-in-sectors) files: the first kind stored as one encrypted unit, the
    /// second as one encrypted zlib sector (with the adjusted key). Built from the format's description.
    fn hand_archive(files: &[(&str, Vec<u8>, bool)], zbig: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; 32];
        let mut blocks = Vec::new();
        for (name, data, sectors) in files {
            let pos = out.len() as u32;
            let base = name.rsplit('\\').next().unwrap();
            if !*sectors {
                let key = hash(base, 3);
                let mut body = data.clone();
                let mut w = words_of(&body);
                encrypt(&mut w, key);
                for (i, v) in w.iter().enumerate() {
                    body[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
                out.extend_from_slice(&body);
                blocks.push([pos, body.len() as u32, data.len() as u32, FLAG_EXISTS | FLAG_SINGLE | FLAG_ENCRYPTED]);
            } else {
                // one sector of 4 KB is too small for 'big': use a bigger sector shift in this archive (16 -> 32 MB)
                let size = data.len() as u32;
                let key = hash(base, 3).wrapping_add(pos) ^ size;
                let mut sec = vec![0x02u8];
                sec.extend_from_slice(zbig);
                let mut table = vec![8u32, 8 + sec.len() as u32];
                encrypt(&mut table, key.wrapping_sub(1));
                let mut body: Vec<u8> = table.iter().flat_map(|v| v.to_le_bytes()).collect();
                let mut w = words_of(&sec);
                encrypt(&mut w, key);
                for (i, v) in w.iter().enumerate() {
                    sec[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
                body.extend_from_slice(&sec);
                out.extend_from_slice(&body);
                blocks.push([pos, body.len() as u32, size, FLAG_EXISTS | FLAG_COMPRESS | FLAG_ENCRYPTED | FLAG_KEY_V2]);
            }
        }
        let size = 16usize;
        let mut table = vec![[0xFFFF_FFFFu32; 4]; size];
        for (i, (n, _, _)) in files.iter().enumerate() {
            let mut at = (hash(n, 0) as usize) & (size - 1);
            while table[at][3] != 0xFFFF_FFFF {
                at = (at + 1) & (size - 1);
            }
            table[at] = [hash(n, 1), hash(n, 2), 0, i as u32];
        }
        let mut hw: Vec<u32> = table.iter().flat_map(|e| e.iter().copied()).collect();
        encrypt(&mut hw, hash("(hash table)", 3));
        let mut bw: Vec<u32> = blocks.iter().flat_map(|e| e.iter().copied()).collect();
        encrypt(&mut bw, hash("(block table)", 3));
        let hpos = out.len() as u32;
        out.extend(hw.iter().flat_map(|v| v.to_le_bytes()));
        let bpos = out.len() as u32;
        out.extend(bw.iter().flat_map(|v| v.to_le_bytes()));
        let total = out.len() as u32;
        let mut h = b"MPQ\x1a".to_vec();
        for v in [32u32, total] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes()); // sector 512 << 16 = 32 MB: one sector holds 'big'
        for v in [hpos, bpos, size as u32, blocks.len() as u32] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        out[..32].copy_from_slice(&h);
        out
    }
}
