//! Game-client downloads from a magnet link or a `.torrent`, for the owner's testing (31 Oct: "magnet links and zip and
//! cloudflare stuff can be dropped in this area … turn off protections for now so i can test").
//!
//! **Off by default.** One switch, Settings › Testing › "Allow torrent downloads of game clients (testing only)",
//! saved on this PC only (`<data dir>/leyline-testing.json`). Off: a magnet or `.torrent` gets today's one sentence
//! (`clientimport::TORRENT_REFUSAL`). On:
//! * a magnet link or `.torrent` the player drops or pastes is downloaded with a real BitTorrent engine (librqbit,
//!   the `torrent-engine` cargo feature): DHT and the torrent's trackers allowed;
//! * upload is off unless "Share while downloading" is on (then capped at `SHARE_CAP_BPS`); when the download is
//!   done the torrent is removed from the engine (files kept), so nothing is seeded afterwards;
//! * files go only into the folder the player picked: every path in the torrent is checked before anything is
//!   written (`mods::safe_rel`: no `..`, no absolute paths, no device names; no symbolic links, no names twice),
//!   with the same size and file-count caps as a zip.
//!
//! Leyline never suggests or pre-fills a source: the link is always the player's own. Leyline's own servers never
//! carry any of it (the site's flags file and the catalog are not involved at all).

use crate::clientimport::{Caps, Job, Phase, GIB};
#[cfg(feature = "torrent-engine")]
use crate::clientimport::CANCELLED;
use crate::p2p::bencode::{self, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

pub const TESTING_FILE: &str = "leyline-testing.json";
/// Upload cap while "Share while downloading" is on: 512 KB/s.
pub const SHARE_CAP_BPS: u32 = 512 * 1024;
/// A `.torrent` of a whole client lists many pieces; this is far above any real one.
pub const MAX_TORRENT_BYTES: u64 = 16 * 1024 * 1024;
/// A magnet link longer than this is refused unread.
pub const MAX_MAGNET: usize = 8192;

// ------------------------------------------------------------------------------------------------ the switch

/// Settings › Testing, saved on this PC only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Testing {
    /// "Allow torrent downloads of game clients (testing only)".
    #[serde(default)]
    pub client_torrents: bool,
    /// "Share while downloading" (upload, capped). Means nothing while `client_torrents` is off.
    #[serde(default)]
    pub share: bool,
}

pub fn testing_path(dir: &Path) -> PathBuf {
    dir.join(TESTING_FILE)
}

/// The switch as saved; a missing or broken file is all off.
pub fn load_testing(dir: &Path) -> Testing {
    fs::read(testing_path(dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_testing(dir: &Path, t: Testing) -> Result<Testing, String> {
    let t = Testing { client_torrents: t.client_torrents, share: t.client_torrents && t.share };
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = testing_path(dir).with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&t).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, testing_path(dir)).map_err(|e| e.to_string())?;
    Ok(t)
}

// ------------------------------------------------------------------------------------------------ the bar's words

/// `1.2 GB`, `850 MB`, `12 KB` (1024-based, as the PC's own file windows count).
pub fn size(b: u64) -> String {
    const MIB: u64 = 1 << 20;
    if b >= GIB {
        format!("{:.1} GB", b as f64 / GIB as f64)
    } else if b >= 100 * MIB {
        format!("{} MB", b / MIB)
    } else if b >= MIB {
        format!("{:.1} MB", b as f64 / MIB as f64)
    } else if b >= 1024 {
        format!("{} KB", b / 1024)
    } else {
        format!("{b} B")
    }
}

/// `14 MB/s`, `1.5 MB/s`, `850 KB/s`.
pub fn speed(bps: u64) -> String {
    const MIB: u64 = 1 << 20;
    if bps >= 10 * MIB {
        format!("{} MB/s", bps / MIB)
    } else if bps >= MIB {
        format!("{:.1} MB/s", bps as f64 / MIB as f64)
    } else {
        format!("{} KB/s", (bps / 1024).max(1))
    }
}

/// Seconds left, rounded up to a minute: `less than a minute left`, `about 4 min left`, `about 1 h 20 min left`.
pub fn eta(secs: u64) -> String {
    if secs < 60 {
        return "less than a minute left".into();
    }
    let min = secs.div_ceil(60);
    if min < 60 {
        format!("about {min} min left")
    } else if min % 60 == 0 {
        format!("about {} h left", min / 60)
    } else {
        format!("about {} h {} min left", min / 60, min % 60)
    }
}

/// Seconds left at this speed, when both are known.
pub fn secs_left(done: u64, total: u64, bps: u64) -> Option<u64> {
    (total > 0 && bps > 0 && done <= total).then(|| (total - done).div_ceil(bps))
}

/// The bar's one line: `Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 4 min left`, `Unpacking · …`,
/// `Checking your game folder…`.
pub fn line(phase: Phase, done: u64, total: u64, bps: u64) -> String {
    let word = match phase {
        Phase::Downloading => "Downloading",
        Phase::Unpacking => "Unpacking",
        Phase::Checking => return "Checking your game folder…".into(),
        _ => return String::new(),
    };
    let mut parts = vec![word.to_string()];
    if total > 0 {
        parts.push(format!("{} / {}", size(done.min(total)), size(total)));
    } else if done > 0 {
        parts.push(size(done));
    } else {
        parts.push("finding the download…".into());
        return parts.join(" · ");
    }
    if bps > 0 && phase == Phase::Downloading {
        parts.push(speed(bps));
    }
    if let Some(s) = secs_left(done, total, bps) {
        parts.push(eta(s));
    }
    parts.join(" · ")
}

/// Bytes per second over the last few seconds.
#[derive(Debug, Clone, Default)]
pub struct Meter {
    samples: VecDeque<(u64, u64)>, // (ms since the phase began, bytes done)
}

impl Meter {
    const WINDOW_MS: u64 = 5000;
    pub fn reset(&mut self) {
        self.samples.clear();
    }
    pub fn push(&mut self, ms: u64, done: u64) {
        if let Some(&(t, _)) = self.samples.back() {
            if ms < t {
                return;
            }
            if ms == t {
                self.samples.pop_back();
            }
        }
        self.samples.push_back((ms, done));
        // keep one sample older than the window, so the span stays about the window
        while self.samples.len() > 2 && ms - self.samples[1].0 >= Self::WINDOW_MS {
            self.samples.pop_front();
        }
    }
    /// 0 until there is at least half a second to go on.
    pub fn bps(&self) -> u64 {
        let (Some(&(t0, d0)), Some(&(t1, d1))) = (self.samples.front(), self.samples.back()) else { return 0 };
        if t1 < t0 + 500 || d1 < d0 {
            return 0;
        }
        (d1 - d0) * 1000 / (t1 - t0)
    }
}

// ------------------------------------------------------------------------------------------------ what was dropped

/// A magnet link the player dropped: its info-hash and name. Only the info-hash is required; the engine reads
/// the rest (trackers, peers) itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMagnet {
    pub info_hash: String,
    pub name: Option<String>,
}

pub fn parse_magnet(uri: &str) -> Result<UserMagnet, String> {
    let bad = || "That magnet link doesn't look right.".to_string();
    let uri = uri.trim();
    if uri.len() > MAX_MAGNET || uri.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad());
    }
    let q = uri.get(..8).filter(|p| p.eq_ignore_ascii_case("magnet:?")).map(|_| &uri[8..]).ok_or_else(bad)?;
    let mut hash = None;
    let mut name = None;
    for kv in q.split('&') {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        let k = k.to_ascii_lowercase();
        if k == "xt" || k.starts_with("xt.") {
            if let Some(h) = v.get(..9).filter(|p| p.eq_ignore_ascii_case("urn:btih:")).map(|_| &v[9..]) {
                let h = crate::p2p::magnet::info_hash(h).ok_or_else(bad)?;
                if hash.as_ref().is_some_and(|x| x != &h) {
                    return Err(bad());
                }
                hash = Some(h);
            }
        } else if k == "dn" && name.is_none() {
            let n = crate::clientimport::percent_decode(&v.replace('+', " "));
            if !n.trim().is_empty() {
                name = Some(n.trim().chars().take(200).collect());
            }
        }
    }
    Ok(UserMagnet { info_hash: hash.ok_or_else(bad)?, name })
}

/// The files a torrent would write: its name, info-hash and each file's path (inside the torrent's own folder
/// for a multi-file torrent) and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub name: String,
    pub info_hash: String,
    pub files: Vec<(String, u64)>,
}

fn utf8(v: &Value) -> Option<String> {
    v.get("name.utf-8").or_else(|| v.get("name")).and_then(Value::str).map(str::to_string)
}

/// Read a `.torrent` (single- or multi-file). Nothing is checked for safety here: `check_files` does that.
pub fn list_torrent(raw: &[u8]) -> Result<Listing, String> {
    let bad = |why: &str| format!("That torrent file can't be read ({why}).");
    if raw.len() as u64 > MAX_TORRENT_BYTES {
        return Err(bad("too big"));
    }
    let (v, info_raw) = bencode::decode(raw).map_err(|e| bad(&e))?;
    let info = v.get("info").ok_or_else(|| bad("no info"))?;
    let info_raw = info_raw.ok_or_else(|| bad("no info"))?;
    let name = utf8(info).filter(|n| !n.is_empty()).ok_or_else(|| bad("no name"))?;
    let mut files = Vec::new();
    match (info.get("files").and_then(Value::list), info.get("length").and_then(Value::int)) {
        (Some(list), _) => {
            for f in list {
                let len = f.get("length").and_then(Value::int).filter(|l| *l >= 0).ok_or_else(|| bad("a file has no size"))? as u64;
                let parts = f.get("path.utf-8").or_else(|| f.get("path")).and_then(Value::list).ok_or_else(|| bad("a file has no path"))?;
                let parts: Option<Vec<&str>> = parts.iter().map(Value::str).collect();
                let parts = parts.ok_or_else(|| bad("a path is not text"))?;
                if f.get("attr").and_then(Value::str).is_some_and(|a| a.contains('l')) || f.get("symlink path").is_some() {
                    return Err("The torrent has a link in it. Nothing was downloaded.".into());
                }
                files.push((format!("{name}/{}", parts.join("/")), len));
            }
        }
        (None, Some(len)) if len >= 0 => files.push((name.clone(), len as u64)),
        _ => return Err(bad("no files")),
    }
    Ok(Listing { name, info_hash: crate::net::hex(&crate::p2p::sha1::digest(info_raw)), files })
}

/// Every path safe to write under the chosen folder, no name twice, within the caps. Returns the total size.
/// Safety, not policy: this holds with the testing switch on.
pub fn check_files(files: &[(String, u64)], caps: &Caps) -> Result<u64, String> {
    if files.is_empty() {
        return Err("The torrent has no files.".into());
    }
    if files.len() > caps.files {
        return Err(format!("The torrent has more than {} files, more than a game client.", caps.files));
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for (path, len) in files {
        let rel = crate::mods::safe_rel(path)
            .ok()
            .flatten()
            .ok_or_else(|| format!("The torrent has an unsafe path in it ({path}). Nothing was downloaded."))?;
        if !seen.insert(rel.to_ascii_lowercase()) {
            return Err(format!("The torrent has {rel} twice. Nothing was downloaded."));
        }
        total = total.saturating_add(*len);
        if total > caps.download {
            return Err(format!("That download is bigger than {} GB, more than a game client.", caps.download / GIB));
        }
    }
    Ok(total)
}

/// What the engine is given.
pub enum Src {
    Magnet(String),
    File(Vec<u8>),
}

/// The words for a failure the engine reports (never the raw error to the player).
pub fn engine_error(e: &str) -> String {
    let l = e.to_ascii_lowercase();
    if l.contains("address family not supported") || l.contains("os error 97") || l.contains("os error 10047") {
        return "This PC's network settings can't run torrent downloads (IPv6 is switched off).".into();
    }
    if l.contains("no space") || l.contains("disk full") || l.contains("os error 28") || l.contains("os error 112") {
        "The disk is full. Free some space or pick another folder, then try again.".into()
    } else if l.contains("permission") || l.contains("access is denied") || l.contains("os error 13") || l.contains("os error 5)") {
        "Leyline can't write to that folder. Pick another one, then try again.".into()
    } else {
        "The torrent download stopped. Try again.".into()
    }
}

#[cfg(not(feature = "torrent-engine"))]
pub fn fetch(_src: Src, _dest: &Path, _caps: &Caps, _share: bool, _job: &Job) -> Result<u64, String> {
    Err("This build of Leyline has no torrent downloads.".into())
}

#[cfg(feature = "torrent-engine")]
pub use engine::fetch;

#[cfg(feature = "torrent-engine")]
mod engine {
    use super::*;
    use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions};
    use std::num::NonZeroU32;
    use std::time::Duration;

    /// How long to look for the torrent's file list (a magnet link) before saying so.
    const FIND_TIMEOUT: Duration = Duration::from_secs(180);

    fn options(share: bool, test_peers: bool) -> SessionOptions {
        let mut o = SessionOptions {
            disable_upload: !share,
            persistence: None,
            fastresume: false,
            client_name_and_version: Some(concat!("Leyline/", env!("CARGO_PKG_VERSION")).into()),
            ..Default::default()
        };
        if let Some(d) = o.dht.as_mut() {
            d.persistence = None;
        }
        if share {
            o.ratelimits.upload_bps = NonZeroU32::new(SHARE_CAP_BPS);
            o.listen = Some(Default::default());
        }
        if test_peers {
            o.dht = None;
            o.disable_trackers = true;
            o.disable_local_service_discovery = true;
        }
        o
    }

    /// Download the torrent into `dest` (made here; it must not exist yet). Its file list is checked first; on
    /// failure or cancel the torrent is dropped and `dest` removed. When done, the engine stops: nothing is
    /// seeded afterwards. Returns the bytes downloaded.
    pub fn fetch(src: Src, dest: &Path, caps: &Caps, share: bool, job: &Job) -> Result<u64, String> {
        fetch_with(src, dest, caps, share, job, &[])
    }

    /// `peers`: extra peers to ask (tests, with DHT and trackers off).
    pub fn fetch_with(src: Src, dest: &Path, caps: &Caps, share: bool, job: &Job, peers: &[std::net::SocketAddr]) -> Result<u64, String> {
        if dest.exists() {
            return Err("That folder is already there.".into());
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| engine_error(&e.to_string()))?;
        let work = dest.with_extension("torrent-work");
        let res = rt.block_on(run(src, dest, &work, caps, share, job, peers));
        let _ = fs::remove_dir_all(&work);
        if res.is_err() {
            let _ = fs::remove_dir_all(dest);
        }
        rt.shutdown_timeout(Duration::from_secs(5));
        res
    }

    async fn run(src: Src, dest: &Path, work: &Path, caps: &Caps, share: bool, job: &Job, peers: &[std::net::SocketAddr]) -> Result<u64, String> {
        job.phase(Phase::Downloading, 0);
        fs::create_dir_all(work).map_err(|e| engine_error(&e.to_string()))?;
        // a PC without IPv6 can't open the engine's [::] sockets: then IPv4 only
        let session = match Session::new_with_opts(work.to_path_buf(), options(share, !peers.is_empty())).await {
            Ok(s) => s,
            Err(_) => Session::new_with_opts(work.to_path_buf(), SessionOptions { ipv4_only: true, ..options(share, !peers.is_empty()) })
                .await
                .map_err(|e| engine_error(&format!("{e:#}")))?,
        };
        let out = (|| async {
            let opts = |list_only: bool| AddTorrentOptions {
                list_only,
                overwrite: false,
                output_folder: Some(dest.display().to_string()),
                initial_peers: (!peers.is_empty()).then(|| peers.to_vec()),
                ..Default::default()
            };
            let first = match &src {
                Src::File(b) => AddTorrent::TorrentFileBytes(b.clone().into()),
                Src::Magnet(m) => AddTorrent::from_url(m.clone()),
            };
            let nobody = || "Nobody is sharing that download right now. Try again later.".to_string();
            // 1. The file list (a magnet: found through DHT and trackers), checked before anything is written.
            let listed = tokio::select! {
                r = session.add_torrent(first, Some(opts(true))) => r.map_err(|e| { let s = format!("{e:#}"); if s.to_ascii_lowercase().contains("timeout") { nobody() } else { engine_error(&s) } })?,
                _ = cancelled(job) => return Err(CANCELLED.to_string()),
                _ = tokio::time::sleep(FIND_TIMEOUT) => return Err(nobody()),
            };
            let AddTorrentResponse::ListOnly(l) = listed else { return Err(engine_error("not listed")) };
            let top = l.info.name().map(|n| n.to_string()).unwrap_or_else(|| "Game client".into());
            let mut files = Vec::new();
            for f in l.info.iter_file_details() {
                if f.symlink_path.is_some() {
                    return Err("The torrent has a link in it. Nothing was downloaded.".into());
                }
                // the torrent's own name is checked as a folder too, though the files go straight into `dest`
                files.push((format!("{top}/{}", f.filename.to_vec().join("/")), f.len));
            }
            let total = check_files(&files, caps)?;
            job.phase(Phase::Downloading, total);
            // 2. The download, into the chosen folder only.
            fs::create_dir_all(dest).map_err(|e| engine_error(&e.to_string()))?;
            let handle = session
                .add_torrent(AddTorrent::TorrentFileBytes(l.torrent_bytes.clone()), Some(opts(false)))
                .await
                .map_err(|e| engine_error(&e.to_string()))?
                .into_handle()
                .ok_or_else(|| engine_error("no handle"))?;
            let id = handle.id();
            loop {
                if job.cancelled() {
                    let _ = session.delete(id.into(), true).await;
                    return Err(CANCELLED.to_string());
                }
                let st = handle.stats();
                job.set_done(st.progress_bytes, st.total_bytes.max(total));
                if let Some(e) = st.error {
                    let _ = session.delete(id.into(), false).await;
                    return Err(engine_error(&e));
                }
                if st.finished {
                    // stop seeding: the torrent leaves the engine, its files stay
                    let _ = session.delete(id.into(), false).await;
                    return Ok(st.progress_bytes);
                }
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
        })()
        .await;
        session.stop().await;
        out
    }

    async fn cancelled(job: &Job) {
        while !job.cancelled() {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::clientimport::CAPS;

        /// A real swarm of two on this PC: one engine seeds a small folder, Leyline's download takes it from that
        /// peer only (no DHT, no trackers), into the chosen folder, and stops when done.
        /// Ignored by default: the engine always opens an IPv6 UDP socket ([::]:0, its tracker client), which a
        /// kernel without IPv6 (this build box) refuses. On a normal PC:
        ///   cargo test --no-default-features --features torrent-engine --lib clienttorrent -- --ignored
        #[test]
        #[ignore = "needs IPv6 sockets"]
        fn downloads_from_a_local_peer_and_stops() {
            let d = tempfile::tempdir().unwrap();
            let seed_dir = d.path().join("seed");
            let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
            fs::create_dir_all(seed_dir.join("Pack").join("Sub")).unwrap();
            fs::write(seed_dir.join("Pack").join("a.bin"), &data).unwrap();
            fs::write(seed_dir.join("Pack").join("Sub").join("b.txt"), b"hello").unwrap();
            let raw = crate::clienttorrent::tests::make_multi("Pack", &[(&["a.bin"], &data), (&["Sub", "b.txt"], b"hello")], 32 * 1024);
            let rt = tokio::runtime::Runtime::new().unwrap();
            let (seeder, addr) = rt.block_on(async {
                let o = SessionOptions {
                    dht: None,
                    disable_trackers: true,
                    disable_local_service_discovery: true,
                    persistence: None,
                    ipv4_only: true,
                    listen: Some(librqbit::ListenerOptions {
                        mode: librqbit::ListenerMode::TcpOnly,
                        listen_addr: "127.0.0.1:0".parse().unwrap(),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                let s = Session::new_with_opts(seed_dir.clone(), o).await.unwrap();
                let h = s
                    .add_torrent(AddTorrent::TorrentFileBytes(raw.clone().into()), Some(AddTorrentOptions { overwrite: true, output_folder: Some(seed_dir.join("Pack").display().to_string()), ..Default::default() }))
                    .await
                    .unwrap()
                    .into_handle()
                    .unwrap();
                h.wait_until_completed().await.unwrap();
                let addr = s.listen_addr().unwrap();
                (s, addr)
            });
            let dest = d.path().join("out").join("Pack");
            let job = Job::default();
            let got = fetch_with(Src::File(raw), &dest, &CAPS, false, &job, &[addr]).unwrap();
            assert_eq!(got, data.len() as u64 + 5);
            assert_eq!(fs::read(dest.join("a.bin")).unwrap(), data);
            assert_eq!(fs::read(dest.join("Sub").join("b.txt")).unwrap(), b"hello");
            let p = job.snapshot();
            assert_eq!((p.done, p.total), (got, got));
            assert!(!dest.with_extension("torrent-work").exists(), "the engine's own folder is gone");
            rt.block_on(seeder.stop());
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::clientimport::{Phase, CAPS};
    use std::collections::BTreeMap;

    fn b(s: &str) -> Value {
        Value::Bytes(s.as_bytes().to_vec())
    }
    fn dict(kv: Vec<(&str, Value)>) -> Value {
        Value::Dict(kv.into_iter().map(|(k, v)| (k.as_bytes().to_vec(), v)).collect::<BTreeMap<_, _>>())
    }
    fn enc(v: &Value, out: &mut Vec<u8>) {
        match v {
            Value::Int(i) => out.extend(format!("i{i}e").bytes()),
            Value::Bytes(x) => {
                out.extend(format!("{}:", x.len()).bytes());
                out.extend(x);
            }
            Value::List(l) => {
                out.push(b'l');
                l.iter().for_each(|x| enc(x, out));
                out.push(b'e');
            }
            Value::Dict(d) => {
                out.push(b'd');
                for (k, x) in d {
                    out.extend(format!("{}:", k.len()).bytes());
                    out.extend(k);
                    enc(x, out);
                }
                out.push(b'e');
            }
        }
    }

    /// A multi-file torrent of `files` under `name` (pieces hashed over the files in order).
    pub(crate) fn make_multi(name: &str, files: &[(&[&str], &[u8])], piece: usize) -> Vec<u8> {
        let all: Vec<u8> = files.iter().flat_map(|(_, d)| d.iter().copied()).collect();
        let pieces: Vec<u8> = all.chunks(piece).flat_map(|c| crate::p2p::sha1::digest(c)).collect();
        let list = files
            .iter()
            .map(|(p, d)| dict(vec![("length", Value::Int(d.len() as i64)), ("path", Value::List(p.iter().map(|s| b(s)).collect()))]))
            .collect();
        let info = dict(vec![("files", Value::List(list)), ("name", b(name)), ("piece length", Value::Int(piece as i64)), ("pieces", Value::Bytes(pieces))]);
        let mut out = Vec::new();
        enc(&dict(vec![("info", info)]), &mut out);
        out
    }

    #[test]
    fn the_switch_is_off_until_turned_on_and_share_needs_it() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load_testing(d.path()), Testing { client_torrents: false, share: false }, "no file: off");
        fs::write(testing_path(d.path()), b"{not json").unwrap();
        assert_eq!(load_testing(d.path()), Testing::default(), "a broken file: off");
        // share alone means nothing
        assert_eq!(save_testing(d.path(), Testing { client_torrents: false, share: true }).unwrap(), Testing::default());
        assert_eq!(load_testing(d.path()), Testing::default());
        save_testing(d.path(), Testing { client_torrents: true, share: false }).unwrap();
        assert_eq!(load_testing(d.path()), Testing { client_torrents: true, share: false });
        save_testing(d.path(), Testing { client_torrents: true, share: true }).unwrap();
        assert!(load_testing(d.path()).share);
    }

    #[test]
    fn progress_words() {
        const MB: u64 = 1 << 20;
        assert_eq!(size(1_288_490_189), "1.2 GB");
        assert_eq!(size(5 * GIB), "5.0 GB");
        assert_eq!(size(850 * MB), "850 MB");
        assert_eq!(size(3 * MB + MB / 2), "3.5 MB");
        assert_eq!(size(12 * 1024), "12 KB");
        assert_eq!(size(7), "7 B");
        assert_eq!(speed(14 * MB + 3), "14 MB/s");
        assert_eq!(speed(MB + MB / 2), "1.5 MB/s");
        assert_eq!(speed(850 * 1024), "850 KB/s");
        assert_eq!(speed(10), "1 KB/s");
        assert_eq!(eta(30), "less than a minute left");
        assert_eq!(eta(60), "about 1 min left");
        assert_eq!(eta(61), "about 2 min left");
        assert_eq!(eta(4 * 60 - 5), "about 4 min left");
        assert_eq!(eta(3600), "about 1 h left");
        assert_eq!(eta(80 * 60), "about 1 h 20 min left");
        // 3.8 GB left at 14 MB/s: about 4.6 min, rounded up
        let (done, total, bps) = (1_288_490_189, 5 * GIB, 14 * MB);
        assert_eq!(secs_left(done, total, bps), Some(278));
        assert_eq!(line(Phase::Downloading, done, total, bps), "Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 5 min left");
        assert_eq!(line(Phase::Downloading, done, total, 0), "Downloading · 1.2 GB / 5.0 GB", "no speed yet: no guess");
        assert_eq!(line(Phase::Downloading, 0, 0, 0), "Downloading · finding the download…");
        assert_eq!(line(Phase::Downloading, 3 * MB, 0, 2 * MB), "Downloading · 3.0 MB · 2.0 MB/s", "size not known: no time left");
        assert_eq!(line(Phase::Unpacking, 2 * GIB, 4 * GIB, 20 * MB), "Unpacking · 2.0 GB / 4.0 GB · about 2 min left");
        assert_eq!(line(Phase::Checking, 0, 0, 0), "Checking your game folder…");
        assert_eq!(line(Phase::Done, 1, 1, 1), "");
        assert_eq!(secs_left(5, 4, 1), None);
    }

    #[test]
    fn speed_over_the_last_seconds() {
        let mut m = Meter::default();
        assert_eq!(m.bps(), 0);
        m.push(0, 0);
        m.push(200, 1000);
        assert_eq!(m.bps(), 0, "under half a second: no number yet");
        for s in 1..=20u64 {
            m.push(s * 1000, s * 1_000_000);
        }
        assert_eq!(m.bps(), 1_000_000);
        // it slows down: the window follows within a few seconds
        for s in 21..=30u64 {
            m.push(s * 1000, 20_000_000 + (s - 20) * 100_000);
        }
        assert_eq!(m.bps(), 100_000);
        m.push(29_000, 0);
        assert_eq!(m.bps(), 100_000, "a sample from the past is ignored");
        m.reset();
        assert_eq!(m.bps(), 0);
    }

    #[test]
    fn reads_magnet_links_loosely_but_needs_the_hash() {
        let h = "0123456789abcdef0123456789abcdef01234567";
        let m = parse_magnet(&format!("magnet:?xt=urn:btih:{}&dn=My+Game%20Folder&tr=udp://t.example:1337&x.pe=1.2.3.4:5&tr=https://t2.example/a", h.to_uppercase())).unwrap();
        assert_eq!(m, UserMagnet { info_hash: h.into(), name: Some("My Game Folder".into()) });
        assert_eq!(parse_magnet("MAGNET:?xt=urn:btih:AERUKZ4JVPG66AJDIVSYSK6NYMAERUKZ").unwrap().info_hash.len(), 40, "base32");
        for bad in [
            "magnet:?dn=x".to_string(),
            "magnet:?xt=urn:btih:xyz".into(),
            format!("magnet:xt=urn:btih:{h}"),
            format!("magnet:?xt=urn:btih:{h} x"),
            format!("magnet:?xt=urn:btih:{h}&xt=urn:btih:{}", "1".repeat(40)),
            format!("magnet:?xt=urn:btih:{h}&{}", "a".repeat(9000)),
        ] {
            assert!(parse_magnet(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reads_torrent_files_and_checks_every_path() {
        let raw = make_multi("My Client", &[(&["Wow.exe"], b"MZ"), (&["Data", "common.MPQ"], b"x")], 16 * 1024);
        let l = list_torrent(&raw).unwrap();
        assert_eq!(l.name, "My Client");
        assert_eq!(l.files, vec![("My Client/Wow.exe".to_string(), 2), ("My Client/Data/common.MPQ".to_string(), 1)]);
        assert_eq!(l.info_hash.len(), 40);
        assert_eq!(check_files(&l.files, &CAPS).unwrap(), 3, "a game client is fine with the switch on: only safety is checked");
        // single-file
        let one = crate::p2p::torrent::make("backup.zip", b"PK\x03\x04", 16 * 1024, &[]);
        assert_eq!(list_torrent(&one).unwrap().files, vec![("backup.zip".to_string(), 4)]);
        assert!(list_torrent(b"d4:spam4:eggse").is_err());
        assert!(list_torrent(b"not bencode").is_err());
        // unsafe paths, before anything is written
        for bad in [&["..", "evil.txt"][..], &["/abs"], &["a", "..", "..", "b"], &["C:", "win"], &["con.txt"], &["ok", ""]] {
            let raw = make_multi("x", &[(bad, b"1")], 16 * 1024);
            let l = list_torrent(&raw).unwrap();
            assert!(check_files(&l.files, &CAPS).unwrap_err().contains("unsafe path"), "{bad:?}");
        }
        let raw = make_multi("..", &[(&["a"], b"1")], 16 * 1024);
        assert!(check_files(&list_torrent(&raw).unwrap().files, &CAPS).is_err(), "the torrent's own name is checked too");
        // names twice (case), caps
        let twice = vec![("x/Data/a".to_string(), 1), ("x/data/A".to_string(), 1)];
        assert!(check_files(&twice, &CAPS).unwrap_err().contains("twice"));
        let caps = Caps { download: 10, unpacked: 10, files: 2 };
        assert!(check_files(&[("a".into(), 11)], &caps).unwrap_err().contains("bigger than"));
        assert!(check_files(&[("a".into(), 1), ("b".into(), 1), ("c".into(), 1)], &caps).unwrap_err().contains("more than 2 files"));
        assert!(check_files(&[], &caps).is_err());
    }

    #[test]
    fn engine_errors_in_plain_words() {
        assert!(engine_error("No space left on device (os error 28)").contains("disk is full"));
        assert!(engine_error("Permission denied (os error 13)").contains("can't write"));
        assert_eq!(engine_error("peer 1.2.3.4 sent garbage at /tmp/x"), "The torrent download stopped. Try again.");
        assert!(engine_error("error creating UDP socket at [::]:0: Address family not supported by protocol (os error 97)").contains("IPv6"));
    }
}
