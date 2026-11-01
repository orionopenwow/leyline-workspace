//! "Help share Leyline's own files": a PC that opts in keeps copies of Leyline's own signed packages and shares
//! them with other players, so Leyline keeps spreading even if GitHub or our server is gone.
//!
//! What may be shared is exactly what the signed release log (`releaselog`, "the chain") names in the kinds
//! `launcher` (the installers), `hostpack`, `addons` and `source` (upstream server SOURCE tarballs at the pinned
//! commits: open-source code, so anyone can still host any supported version if upstream disappears). A file
//! is kept only when its sha256 is the one in the log, it passes the refusal list (`checker`: never game data,
//! never a game client) and it is in this PC's plan; anything else in the folder is removed. Nothing a player
//! drops, pastes or downloads for themselves is ever shared from here.
//!
//! Off three ways by default (`flags::share_allowed`): the site must offer it (the owner's switch for everyone),
//! this PC must turn it on (Host > Advanced, "Help share Leyline's own files"), and the upload cap must be above
//! zero. Turned off, the folder is emptied.
//!
//! How: files come in through `transfer::fetch` (the mirrors the log names as web seeds, each piece and the whole
//! file checked); each gets a `.torrent` made here with the same rules as the server's tools (single file, 256 KiB
//! pieces), whose info-hash must equal the one in the log; with the `torrent-engine` feature the same engine as
//! clienttorrent.rs seeds them, upload capped. Without it, the plan and the checked copies are kept and nothing
//! is uploaded.

use super::bencode::{self, Value};
use super::checker;
use super::flags::Flags;
use super::transfer::{self, Kind, Request, Swarm};
use crate::releaselog::{Chain, Entry, FileRef};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const DIR: &str = "share";
pub const SHAREABLE: &[&str] = &["launcher", "hostpack", "addons", "source"];
/// At most this much disk for shared copies (the plan leaves out what does not fit).
pub const DISK_CAP: u64 = 3 * 1024 * 1024 * 1024;
/// One file is at most this big.
pub const MAX_FILE: u64 = 1024 * 1024 * 1024;
/// Piece length of the torrents: the same as hosting/pack/make-hostpack.py and tools/release-mirror.
pub const PIECE: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub kind: String,
    /// The entry's name ("Leyline", "Host pack", "mangos-classic").
    pub entry: String,
    pub version: String,
    pub file: FileRef,
    /// Where to fetch it: each mirror the log names, at `<mirror><entry_dir>/<file name>`.
    pub urls: Vec<String>,
    pub infohash: Option<String>,
}

pub fn dir(app_dir: &Path) -> PathBuf {
    app_dir.join(DIR)
}

/// The folder an entry's files sit in on every mirror (tools/release-mirror uses the same layout).
pub fn entry_dir(e: &Entry) -> String {
    match e.kind.as_str() {
        "launcher" => format!("{}{}", crate::update::TAG_PREFIX, e.version),
        "hostpack" => "host-pack".into(),
        "addons" => "addons".into(),
        _ => "sources".into(),
    }
}

/// Payloads only: the manifests and .torrent files that sit beside them are not worth seeding.
fn payload(f: &FileRef) -> bool {
    let n = f.name.to_ascii_lowercase();
    !(n.ends_with(".torrent") || n.ends_with(".json") || n.ends_with(".sig"))
}

/// What this PC would share: the newest entry per (kind, name) of the shareable kinds, source code first (it is
/// what is hardest to find again), then the Host pack, the addons and the installers, within `cap` bytes.
pub fn plan(chain: &Chain, cap: u64) -> Vec<Item> {
    let mut newest: BTreeMap<(String, String), &Entry> = BTreeMap::new();
    for e in &chain.entries {
        if SHAREABLE.contains(&e.kind.as_str()) {
            newest.insert((e.kind.clone(), e.name.clone()), e); // later entries replace earlier ones
        }
    }
    let rank = |k: &str| SHAREABLE.iter().rev().position(|x| *x == k).unwrap_or(9);
    let mut ents: Vec<&Entry> = newest.into_values().collect();
    ents.sort_by_key(|e| (rank(&e.kind), std::cmp::Reverse(e.seq)));
    let (mut out, mut used) = (Vec::new(), 0u64);
    let mut seen = HashSet::new();
    for e in ents {
        for f in e.files.iter().filter(|f| payload(f) && f.size <= MAX_FILE) {
            if used + f.size > cap || !seen.insert(f.sha256.clone()) || checker::name_refusal(&f.name).is_some() {
                continue;
            }
            used += f.size;
            let urls = e.mirrors.iter().map(|m| format!("{m}{}/{}", entry_dir(e), f.name)).collect();
            // the log's info-hash belongs to the entry's single payload; with several payloads there is none
            let one = e.files.iter().filter(|x| payload(x)).count() == 1;
            out.push(Item { kind: e.kind.clone(), entry: e.name.clone(), version: e.version.clone(), file: f.clone(), urls, infohash: e.infohash.clone().filter(|_| one) });
        }
    }
    out
}

/// The allow-list: a file may be shared only when the log names it with this sha256 in a shareable kind.
pub fn allowed(chain: &Chain, name: &str, sha256: &str) -> bool {
    chain.files().any(|(e, f)| SHAREABLE.contains(&e.kind.as_str()) && f.name == name && f.sha256 == sha256) && checker::name_refusal(name).is_none()
}

/// Remove everything in the share folder that is not in the plan with the right size and sha256. Returns the
/// items that are there and good.
pub fn tidy(share_dir: &Path, plan: &[Item]) -> Vec<Item> {
    let want: BTreeMap<&str, &Item> = plan.iter().map(|i| (i.file.name.as_str(), i)).collect();
    let mut good = Vec::new();
    for ent in fs::read_dir(share_dir).into_iter().flatten().flatten() {
        let p = ent.path();
        let name = ent.file_name().to_string_lossy().to_string();
        let keep = p.is_file()
            && want.get(name.as_str()).is_some_and(|i| {
                fs::metadata(&p).map(|m| m.len()).ok() == Some(i.file.size) && crate::net::sha256_file(&p).ok().as_deref() == Some(i.file.sha256.as_str())
            });
        if keep {
            good.push((*want[name.as_str()]).clone());
        } else if p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        } else {
            let _ = fs::remove_file(&p);
        }
    }
    good.sort_by(|a, b| a.file.name.cmp(&b.file.name));
    good
}

/// Fetch what the plan has and the folder lacks, through the one checked download path. Returns one line per
/// file fetched or failed.
pub fn fill(share_dir: &Path, plan: &[Item], flags: &Flags, swarm: &dyn Swarm) -> Vec<String> {
    let _ = fs::create_dir_all(share_dir);
    let have: HashSet<String> = tidy(share_dir, plan).into_iter().map(|i| i.file.name).collect();
    let mut notes = Vec::new();
    for i in plan.iter().filter(|i| !have.contains(&i.file.name)) {
        let dest = share_dir.join(&i.file.name);
        let mut last = "no mirror names it".to_string();
        let mut done = false;
        for url in &i.urls {
            let req = Request { kind: Kind::SharedFile, name: i.file.name.clone(), sha256: i.file.sha256.clone(), https_url: url.clone(), torrent: None, max_bytes: i.file.size.min(MAX_FILE) };
            match transfer::fetch(&req, &dest, flags, swarm) {
                Ok(_) => {
                    done = true;
                    break;
                }
                Err(e) => last = e,
            }
        }
        notes.push(if done { format!("{}: fetched", i.file.name) } else { format!("{}: {last}", i.file.name) });
    }
    notes
}

/// A single-file `.torrent` for a file on disk (read piece by piece), with web seeds: the same bytes as
/// `torrent::make` would give for the whole file in memory.
pub fn torrent_of_file(path: &Path, name: &str, web_seeds: &[String]) -> Result<Vec<u8>, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let (mut pieces, mut len, mut buf) = (Vec::new(), 0u64, vec![0u8; PIECE as usize]);
    loop {
        let mut got = 0;
        while got < buf.len() {
            let n = f.read(&mut buf[got..]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            got += n;
        }
        if got == 0 {
            break;
        }
        pieces.extend_from_slice(&super::sha1::digest(&buf[..got]));
        len += got as u64;
        if got < buf.len() {
            break;
        }
    }
    let mut info = BTreeMap::new();
    info.insert(b"length".to_vec(), Value::Int(len as i64));
    info.insert(b"name".to_vec(), Value::Bytes(name.as_bytes().to_vec()));
    info.insert(b"piece length".to_vec(), Value::Int(PIECE as i64));
    info.insert(b"pieces".to_vec(), Value::Bytes(pieces));
    let mut top = BTreeMap::new();
    top.insert(b"info".to_vec(), Value::Dict(info));
    if !web_seeds.is_empty() {
        top.insert(b"url-list".to_vec(), Value::List(web_seeds.iter().map(|s| Value::Bytes(s.as_bytes().to_vec())).collect()));
    }
    Ok(bencode::encode(&Value::Dict(top)))
}

/// The torrents to seed: one per good file, each checked again (allow-list, refusal list, info-hash = the log's).
pub fn seedable(share_dir: &Path, chain: &Chain, good: &[Item]) -> Vec<(Item, Vec<u8>)> {
    good.iter()
        .filter(|i| allowed(chain, &i.file.name, &i.file.sha256))
        .filter(|i| checker::check_file(&share_dir.join(&i.file.name), &i.file.name).is_ok())
        .filter_map(|i| {
            let t = torrent_of_file(&share_dir.join(&i.file.name), &i.file.name, &i.urls).ok()?;
            let ih = super::torrent::parse(&t).ok()?.info_hash_hex();
            (i.infohash.is_none() || i.infohash.as_deref() == Some(ih.as_str())).then(|| (i.clone(), t))
        })
        .collect()
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Status {
    pub on: bool,
    pub files: Vec<Item>,
    pub notes: Vec<String>,
    /// True when this build can upload (the torrent engine is built in).
    pub engine: bool,
}

/// One pass (the app runs it with each update check): plan, tidy, fetch, and the list to seed. Off: the folder
/// is emptied. `chain` is the verified release log (None without a trusted key: nothing is shared).
pub fn pass(app_dir: &Path, chain: Option<&Chain>, on: bool, flags: &Flags, swarm: &dyn Swarm) -> (Status, Vec<(Item, Vec<u8>)>) {
    let d = dir(app_dir);
    let engine = cfg!(feature = "torrent-engine");
    let (Some(chain), true) = (chain, on) else {
        let _ = fs::remove_dir_all(&d);
        return (Status { on: false, engine, ..Default::default() }, vec![]);
    };
    let p = plan(chain, DISK_CAP);
    let notes = fill(&d, &p, flags, swarm);
    let good = tidy(&d, &p);
    let seeds = seedable(&d, chain, &good);
    (Status { on: true, files: good, notes, engine }, seeds)
}

/// The app's pass (with each update check, on its own thread): the flags, the verified log, `pass`, and with the
/// torrent engine the seeder restarted when the set of files changed (stopped when sharing is off). One pass at a
/// time.
pub fn run(app_dir: &Path) -> Status {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Status { notes: vec!["already running".into()], ..Default::default() };
    }
    let site = crate::net::read_cache(&super::flags::cache_path(app_dir))
        .filter(|(v, _)| super::flags::check(v).is_ok())
        .map(|(v, _)| super::flags::from_doc(&v))
        .unwrap_or_default();
    let local = super::flags::load_local(app_dir);
    let on = super::flags::share_allowed(&site, &local);
    let flags = super::flags::effective(&site, &local);
    let chain = crate::releaselog::trusted_key().map(|k| crate::releaselog::load(app_dir, k));
    let (st, seeds) = pass(app_dir, chain.as_ref(), on, &flags, &transfer::NoSwarm);
    #[cfg(feature = "torrent-engine")]
    seeder::set(app_dir, seeds, flags.limits.seed_upload_kbps);
    #[cfg(not(feature = "torrent-engine"))]
    drop(seeds);
    RUNNING.store(false, Ordering::SeqCst);
    st
}

#[cfg(feature = "torrent-engine")]
mod seeder {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    static NOW: Mutex<Option<(Vec<String>, Arc<AtomicBool>)>> = Mutex::new(None);

    /// Seed exactly `seeds` (an empty list stops seeding).
    pub fn set(app_dir: &Path, seeds: Vec<(Item, Vec<u8>)>, upload_kbps: u32) {
        let names: Vec<String> = seeds.iter().map(|(i, _)| format!("{}:{}", i.file.name, i.file.sha256)).collect();
        let Ok(mut now) = NOW.lock() else { return };
        if now.as_ref().map(|(n, _)| n) == Some(&names) {
            return;
        }
        if let Some((_, stop)) = now.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if seeds.is_empty() || upload_kbps == 0 {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *now = Some((names, stop.clone()));
        let d = dir(app_dir);
        std::thread::spawn(move || {
            if let Err(e) = engine::seed(&d, seeds, upload_kbps, stop) {
                eprintln!("share: the engine stopped: {e}");
            }
        });
    }
}

#[cfg(feature = "torrent-engine")]
pub mod engine {
    //! Seeding with the librqbit engine (the same one clienttorrent.rs uses), upload capped, for the torrents
    //! `seedable` returned and nothing else. Runs until `stop` is set.
    use super::*;
    use librqbit::{AddTorrent, AddTorrentOptions, Session, SessionOptions};
    use std::num::NonZeroU32;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    pub fn seed(share_dir: &Path, torrents: Vec<(Item, Vec<u8>)>, upload_kbps: u32, stop: Arc<AtomicBool>) -> Result<(), String> {
        if torrents.is_empty() || upload_kbps == 0 {
            return Ok(());
        }
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())?;
        let work = share_dir.join(".engine");
        let _ = fs::create_dir_all(&work);
        let res = rt.block_on(async {
            let opts = |ipv4_only: bool| {
                let mut o = SessionOptions { disable_upload: false, persistence: None, fastresume: false, ipv4_only,
                    client_name_and_version: Some(concat!("Leyline/", env!("CARGO_PKG_VERSION")).into()), ..Default::default() };
                if let Some(d) = o.dht.as_mut() {
                    d.persistence = None;
                }
                o.ratelimits.upload_bps = NonZeroU32::new(upload_kbps.saturating_mul(1024));
                o.listen = Some(Default::default());
                o
            };
            // a PC without IPv6 can't open the engine's [::] sockets: then IPv4 only
            let session = match Session::new_with_opts(work.clone(), opts(false)).await {
                Ok(s) => s,
                Err(_) => Session::new_with_opts(work.clone(), opts(true)).await.map_err(|e| format!("{e:#}"))?,
            };
            for (item, t) in &torrents {
                let opts = AddTorrentOptions { overwrite: true, output_folder: Some(share_dir.display().to_string()), ..Default::default() };
                if let Err(e) = session.add_torrent(AddTorrent::TorrentFileBytes(t.clone().into()), Some(opts)).await {
                    eprintln!("share: {}: {e:#}", item.file.name);
                }
            }
            while !stop.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            session.stop().await;
            Ok::<(), String>(())
        });
        let _ = fs::remove_dir_all(&work);
        rt.shutdown_timeout(Duration::from_secs(5));
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::testserver::*;
    use crate::releaselog::tests::{append, pub_hex, DEV_TEST_SEED};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn sha(b: &[u8]) -> String {
        crate::net::hex(&Sha256::digest(b))
    }

    fn log_with(files: &[(&str, &str, &[u8])], mirror: &str) -> Chain {
        let mut d = json!({"format": 1, "key": pub_hex(DEV_TEST_SEED), "entries": []});
        for (kind, name, data) in files {
            let extra = if *kind == "source" {
                json!({"mirrors": [mirror], "source": {"repo": "https://github.com/cmangos/mangos-classic", "commit": "5731249195520323cb5fb9aa29761d4f033272ce"}})
            } else {
                json!({"mirrors": [mirror]})
            };
            let ver = if *kind == "launcher" { "0.3.2" } else { "1" };
            let entry = if *kind == "source" { "mangos-classic" } else { kind };
            d = append(&d, DEV_TEST_SEED, kind, entry, ver, json!([{"name": name, "size": data.len(), "sha256": sha(data)}]), extra);
        }
        crate::releaselog::verify(&d, &pub_hex(DEV_TEST_SEED)).unwrap()
    }

    #[test]
    fn the_plan_is_the_logs_files_source_first_within_the_cap() {
        let c = log_with(&[("launcher", "Leyline_0.3.2_x64-setup.exe", b"exe"), ("hostpack", "leyline-host-pack-1.zip", b"zip!"),
                           ("source", "mangos-classic-5731249.tar.gz", b"tar.gz"), ("note", "note.txt", b"n")], "https://m.example.org/r/");
        let p = plan(&c, DISK_CAP);
        let names: Vec<&str> = p.iter().map(|i| i.file.name.as_str()).collect();
        assert_eq!(names, ["mangos-classic-5731249.tar.gz", "leyline-host-pack-1.zip", "Leyline_0.3.2_x64-setup.exe"], "notes are not shared");
        assert_eq!(p[2].urls, ["https://m.example.org/r/launcher-v0.3.2/Leyline_0.3.2_x64-setup.exe"]);
        assert_eq!(p[0].urls, ["https://m.example.org/r/sources/mangos-classic-5731249.tar.gz"]);
        // a small cap keeps the first that fit
        let p = plan(&c, 7);
        assert_eq!(p.len(), 1);
        assert!(allowed(&c, "leyline-host-pack-1.zip", &sha(b"zip!")));
        assert!(!allowed(&c, "leyline-host-pack-1.zip", &sha(b"other bytes")), "the log's hash only");
        assert!(!allowed(&c, "note.txt", &sha(b"n")), "not a shareable kind");
        assert!(!allowed(&c, "anything.zip", &sha(b"zip!")));
    }

    #[test]
    fn fills_from_the_mirror_tidies_strangers_and_seeds_only_checked_files() {
        let pack = crate::mods::tests::zip_of(&[("host/leyline", b"#!/bin/sh\n")]);
        let src = checker::tests::targz_of(&[("mangos-classic-x/src/a.cpp", b"int a;")]);
        let base = serve_routes(vec![
            ("/r/host-pack/leyline-host-pack-1.zip", "200 OK", pack.clone()),
            ("/r/sources/mangos-classic-5731249.tar.gz", "200 OK", src.clone()),
        ]);
        // the release log's mirrors are https; the test server is http, so build the plan by hand from a log
        let c = log_with(&[("hostpack", "leyline-host-pack-1.zip", &pack), ("source", "mangos-classic-5731249.tar.gz", &src)], "https://m.example.org/r/");
        let mut p = plan(&c, DISK_CAP);
        for i in &mut p {
            i.urls = vec![format!("{base}/r/{}/{}", if i.kind == "source" { "sources" } else { "host-pack" }, i.file.name)];
        }
        let d = tempfile::tempdir().unwrap();
        let sd = d.path().join(DIR);
        fs::create_dir_all(&sd).unwrap();
        fs::write(sd.join("stranger.zip"), b"not ours").unwrap();
        fs::write(sd.join("leyline-host-pack-1.zip"), b"wrong bytes").unwrap();
        let notes = fill(&sd, &p, &Flags::default(), &transfer::NoSwarm);
        assert!(notes.iter().all(|n| n.ends_with("fetched")), "{notes:?}");
        let good = tidy(&sd, &p);
        assert_eq!(good.len(), 2);
        assert!(!sd.join("stranger.zip").exists(), "anything not in the plan is removed");
        let seeds = seedable(&sd, &c, &good);
        assert_eq!(seeds.len(), 2);
        // the torrent made from disk is the same as the in-memory one, with the same info-hash
        let mem = super::super::torrent::make("leyline-host-pack-1.zip", &pack, PIECE, &[]);
        let disk = torrent_of_file(&sd.join("leyline-host-pack-1.zip"), "leyline-host-pack-1.zip", &[]).unwrap();
        assert_eq!(mem, disk);
        // a file the log does not name (someone dropped it in with the right name but other bytes) is never seeded
        fs::write(sd.join("leyline-host-pack-1.zip"), b"PK\x03\x04 swapped").unwrap();
        let good2: Vec<Item> = good.iter().map(|i| { let mut j = i.clone(); if j.kind == "hostpack" { j.file.sha256 = sha(b"PK\x03\x04 swapped"); } j }).collect();
        assert_eq!(seedable(&sd, &c, &good2).len(), 1, "only the tarball still matches the log");
    }

    #[test]
    fn a_wrong_infohash_is_not_seeded_and_off_empties_the_folder() {
        let pack = crate::mods::tests::zip_of(&[("host/leyline", b"#!/bin/sh\n")]);
        let mut d = json!({"format": 1, "key": pub_hex(DEV_TEST_SEED), "entries": []});
        d = append(&d, DEV_TEST_SEED, "hostpack", "Host pack", "1", json!([{"name": "p.zip", "size": pack.len(), "sha256": sha(&pack)}]),
                   json!({"infohash": "00".repeat(20)}));
        let c = crate::releaselog::verify(&d, &pub_hex(DEV_TEST_SEED)).unwrap();
        let t = tempfile::tempdir().unwrap();
        let sd = dir(t.path());
        fs::create_dir_all(&sd).unwrap();
        fs::write(sd.join("p.zip"), &pack).unwrap();
        let good = tidy(&sd, &plan(&c, DISK_CAP));
        assert_eq!(good.len(), 1);
        assert!(seedable(&sd, &c, &good).is_empty(), "the torrent must hash to the log's info-hash");
        let (st, seeds) = pass(t.path(), Some(&c), false, &Flags::default(), &transfer::NoSwarm);
        assert!(!st.on && seeds.is_empty() && !sd.exists(), "turned off: nothing kept");
        let (st, _) = pass(t.path(), None, true, &Flags::default(), &transfer::NoSwarm);
        assert!(!st.on, "no trusted log: nothing shared");
    }

    #[test]
    fn sharing_is_off_until_the_site_offers_it_and_this_pc_turns_it_on() {
        use super::super::flags::{share_allowed, Limits, Local};
        let site_on = Flags { share_own_files: true, ..Default::default() };
        let mut local = Local::default();
        assert!(!share_allowed(&Flags::default(), &local));
        assert!(!share_allowed(&site_on, &local), "the site alone never starts it");
        local.overrides.insert("share_own_files".into(), true);
        assert!(!share_allowed(&site_on, &local), "needs an upload cap");
        local.limits = Some(Limits { seed_upload_kbps: 256, seed_ratio_pct: 0, max_peers: 20 });
        assert!(share_allowed(&site_on, &local));
        assert!(!share_allowed(&Flags::default(), &local), "the owner's switch stops it everywhere");
    }
}
