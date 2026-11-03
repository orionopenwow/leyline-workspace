//! The game's own settings kept for it: key bindings, macros, chat layout (the client's "account data").
//!
//! 2 Nov 2026: in Your Den the character never moved, with the test's keys or a real keyboard, and the game folder had no
//! bindings-cache.wtf at all. The game asks the server for its account data (CMSG_REQUEST_ACCOUNT_DATA) when the
//! server's times say there is some; Den Lite never answered, so the game had no key bindings: W did nothing.
//! Now every ask gets an answer (empty the first time: the game then uses its default keys) and what the game
//! uploads is kept in `<dens>/account-data/`, so the keys someone sets stay the next time.
//!
//! Raw bodies (3.3.5a, 12340):
//! - CMSG_REQUEST_ACCOUNT_DATA 0x20A: u32 type
//! - CMSG_UPDATE_ACCOUNT_DATA 0x20B: u32 type, u32 time, u32 decompressed size, zlib bytes
//! - SMSG_UPDATE_ACCOUNT_DATA 0x20C: u64 guid, u32 type, u32 time, u32 decompressed size, zlib bytes
//! - SMSG_UPDATE_ACCOUNT_DATA_COMPLETE 0x463: u32 type, u32 0

use std::path::{Path, PathBuf};

pub const CMSG_REQUEST: u32 = 0x020A;
pub const CMSG_UPDATE: u32 = 0x020B;
pub const SMSG_UPDATE: u16 = 0x020C;
pub const SMSG_COMPLETE: u16 = 0x0463;
/// Types 0-7; the odd ones (1, 3, 5, 7) belong to one character.
pub const TYPES: u32 = 8;

pub fn per_character(t: u32) -> bool {
    t % 2 == 1
}

fn safe(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).take(32).collect::<String>().to_uppercase()
}

/// `<dens>/account-data/<ACCOUNT>-<type>.bin` (or `<ACCOUNT>-<CHARACTER>-<type>.bin` for a character's own).
pub fn file(dir: &Path, account: &str, character: &str, t: u32) -> PathBuf {
    let name = if per_character(t) { format!("{}-{}-{t}.bin", safe(account), safe(character)) } else { format!("{}-{t}.bin", safe(account)) };
    dir.join("account-data").join(name)
}

/// What is kept: (time, decompressed size, zlib bytes), or None.
pub fn load(dir: &Path, account: &str, character: &str, t: u32) -> Option<(u32, u32, Vec<u8>)> {
    let b = std::fs::read(file(dir, account, character, t)).ok()?;
    if b.len() < 8 {
        return None;
    }
    let time = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    let size = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
    Some((time, size, b[8..].to_vec()))
}

/// The game's upload (CMSG_UPDATE_ACCOUNT_DATA's body): kept; returns its type.
pub fn store(dir: &Path, account: &str, character: &str, body: &[u8]) -> Option<u32> {
    if body.len() < 12 || body.len() > 70_000 {
        return None;
    }
    let t = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    if t >= TYPES {
        return None;
    }
    let f = file(dir, account, character, t);
    if let Some(p) = f.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let tmp = f.with_extension("tmp");
    if std::fs::write(&tmp, &body[4..]).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
    Some(t)
}

/// The answer to CMSG_REQUEST_ACCOUNT_DATA (empty when nothing is kept: the game then uses its defaults).
pub fn answer(dir: &Path, account: &str, character: &str, guid: u64, t: u32) -> Vec<u8> {
    let (time, size, data) = load(dir, account, character, t).unwrap_or((0, 0, Vec::new()));
    let mut v = Vec::with_capacity(20 + data.len());
    v.extend_from_slice(&(if per_character(t) { guid } else { 0 }).to_le_bytes());
    v.extend_from_slice(&t.to_le_bytes());
    v.extend_from_slice(&time.to_le_bytes());
    v.extend_from_slice(&size.to_le_bytes());
    if size > 0 {
        v.extend_from_slice(&data);
    }
    v
}

/// The kept times for a mask (bit i: type i), for SMSG_ACCOUNT_DATA_TIMES (0: nothing kept).
pub fn times(dir: &Path, account: &str, character: &str, mask: u32) -> [u32; 8] {
    let mut out = [0u32; 8];
    for (t, o) in out.iter_mut().enumerate() {
        if mask & (1 << t) != 0 {
            *o = load(dir, account, character, t as u32).map(|x| x.0).unwrap_or(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_with_nothing_kept_gets_an_empty_answer_and_an_upload_comes_back() {
        let dir = std::env::temp_dir().join(format!("den-accdata-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // nothing kept: guid 0 (account-wide), type 0, time 0, size 0
        let a = answer(&dir, "den", "Wren", 0x26a, 0);
        assert_eq!(a, vec![0u8; 20]);
        // the game uploads its key bindings (type 4, account-wide)
        let mut up = Vec::new();
        up.extend_from_slice(&4u32.to_le_bytes());
        up.extend_from_slice(&1_793_600_000u32.to_le_bytes());
        up.extend_from_slice(&5u32.to_le_bytes());
        up.extend_from_slice(&[1, 2, 3]);
        assert_eq!(store(&dir, "den", "Wren", &up), Some(4));
        let a = answer(&dir, "DEN", "wren", 0x26a, 4);
        assert_eq!(&a[0..8], &0u64.to_le_bytes());
        assert_eq!(&a[8..12], &4u32.to_le_bytes());
        assert_eq!(&a[12..16], &1_793_600_000u32.to_le_bytes());
        assert_eq!(&a[16..20], &5u32.to_le_bytes());
        assert_eq!(&a[20..], &[1, 2, 3]);
        assert_eq!(times(&dir, "den", "Wren", 0x15)[4], 1_793_600_000);
        // a character's own (type 5) carries the character's guid and is kept apart
        up[0..4].copy_from_slice(&5u32.to_le_bytes());
        store(&dir, "den", "Wren", &up);
        assert_eq!(&answer(&dir, "den", "Wren", 0x26a, 5)[0..8], &0x26au64.to_le_bytes());
        assert_eq!(answer(&dir, "den", "Rowan", 0x26b, 5)[16..20], 0u32.to_le_bytes());
        // nonsense is not kept
        assert_eq!(store(&dir, "den", "Wren", &[9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
