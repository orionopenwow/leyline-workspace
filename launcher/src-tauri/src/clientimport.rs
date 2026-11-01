//! Adding a game client by dropping it on the launcher (owner 31 Oct: "allow drag and drop download links … to be
//! dropped on the game client drop up window and show loading bar within the launcher").
//!
//! Three things can be dropped (or pasted) on the play bar's client list or the Find your game files guide:
//! * a **folder**: the player's own client, added where it is (the folder itself or one up to three levels inside);
//! * a **zip** of it: unpacked into a folder the player picks (by default `<data dir>/Game clients/<zip name>`);
//! * a **direct https link** to the player's own backup zip: downloaded, then unpacked the same way.
//!
//! Then `clients::detect` looks at what is there and the client is added like one picked with Choose a folder.
//!
//! Never a magnet link or a .torrent, unless the owner's testing switch is on (Settings › Testing, off by default,
//! `clienttorrent.rs`): Leyline never moves game clients peer-to-peer (docs/P2P-AND-TORRENT.md), so with the switch
//! off those get one plain sentence (`TORRENT_REFUSAL`). Leyline never suggests or lists where to get a client;
//! the link is always the player's own.
//!
//! The play bar's realm box takes drops too (owner 31 Oct): it asks where the game should go, turns into the
//! progress bar (`Progress::line`: "Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 4 min left"), then picks the
//! new client for every realm of its version.
//!
//! Unpacking is careful: every path is checked before anything is written (no `..`, no absolute paths, no
//! symbolic links: zip-slip), with caps on the download, the unpacked size and the file count; a file that
//! unpacks bigger than it says is refused. Cancel stops between chunks and removes what this import wrote.

use crate::clients::{self, Detection};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const TORRENT_REFUSAL: &str = "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.";
pub const CANCELLED: &str = "Stopped. Nothing was added.";
const NOT_A_CLIENT: &str = "No game client found in it. The right folder holds the game's program and a Data folder.";
const UNPACKED_NOT_A_CLIENT: &str = "It is unpacked, but no game client is in it. The right folder holds the game's program and a Data folder.";
/// Where zips are unpacked unless the player picks another folder: `<data dir>/Game clients`.
pub const DEFAULT_FOLDER: &str = "Game clients";

/// The most an import may take.
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub download: u64,
    pub unpacked: u64,
    pub files: usize,
}

pub(crate) const GIB: u64 = 1 << 30;
/// A whole client is up to about 20 GB; these leave room and still stop a runaway download or a zip bomb.
pub const CAPS: Caps = Caps { download: 64 * GIB, unpacked: 128 * GIB, files: 400_000 };

/// What was dropped.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Source {
    Folder { path: PathBuf },
    Zip { path: PathBuf },
    Link { url: String },
    /// A magnet link (only with the testing switch on).
    Magnet { uri: String, name: Option<String> },
    /// A `.torrent` file on this PC (only with the testing switch on).
    Torrent { path: PathBuf },
    /// An https link to a `.torrent` (only with the testing switch on).
    TorrentLink { url: String },
}

impl Source {
    /// A short name for the panel: the folder or zip's name, or the link's site.
    pub fn name(&self) -> String {
        match self {
            Source::Folder { path } | Source::Zip { path } => path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            Source::Link { url } | Source::TorrentLink { url } => crate::net::origin(url).map(|o| o.trim_start_matches("https://").to_string()).unwrap_or_default(),
            Source::Torrent { path } => path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            Source::Magnet { name, .. } => name.clone().unwrap_or_else(|| "the shared link".into()),
        }
    }
    /// A zip or a link is unpacked into a folder; a folder is added where it is.
    pub fn unpacks(&self) -> bool {
        !matches!(self, Source::Folder { .. })
    }
}

fn is_torrent(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.starts_with("magnet:") || l.split(['?', '#']).next().unwrap_or("").ends_with(".torrent")
}

/// `%20` and friends in a file:// address.
pub(crate) fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Say what a dropped or pasted text is: a folder, a zip, or a direct https link. The first usable line counts
/// (a dropped link can come as a text/uri-list with `#` comment lines). A magnet or `.torrent` is refused
/// (`TORRENT_REFUSAL`): `classify_with(input, true)` takes them, for the testing switch.
pub fn classify(input: &str) -> Result<Source, String> {
    classify_with(input, false)
}

/// `classify`, with magnets and `.torrent`s taken when `torrents` (Settings › Testing) is on.
pub fn classify_with(input: &str, torrents: bool) -> Result<Source, String> {
    let line = input.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')).unwrap_or("").trim_matches('"');
    if line.is_empty() {
        return Err("Drop your game folder or a zip of it here, or paste a link to your own backup.".into());
    }
    let lower = line.to_ascii_lowercase();
    if is_torrent(line) {
        if !torrents {
            return Err(TORRENT_REFUSAL.into());
        }
        if lower.starts_with("magnet:") {
            let m = crate::clienttorrent::parse_magnet(line)?;
            return Ok(Source::Magnet { uri: line.to_string(), name: m.name });
        }
        if lower.starts_with("https://") {
            if !crate::net::plain_https(line) {
                return Err("That link doesn't look right. Use a direct https link.".into());
            }
            return Ok(Source::TorrentLink { url: line.to_string() });
        }
    }
    if lower.starts_with("http://") {
        return Err("Use a link that starts with https.".into());
    }
    if lower.starts_with("https://") {
        if !crate::net::plain_https(line) {
            return Err("That link doesn't look right. Use a direct https link to your zip.".into());
        }
        return Ok(Source::Link { url: line.to_string() });
    }
    let path = if lower.starts_with("file://") {
        let p = percent_decode(&line[7..]);
        let p = p.strip_prefix("localhost").unwrap_or(&p).to_string();
        // file:///C:/Games -> C:/Games on Windows
        if p.len() > 2 && p.as_bytes()[0] == b'/' && p.as_bytes()[2] == b':' {
            PathBuf::from(&p[1..])
        } else {
            PathBuf::from(p)
        }
    } else if lower.contains("://") {
        return Err("Leyline can take a folder, a zip, or a direct https link.".into());
    } else {
        PathBuf::from(line)
    };
    if !path.is_absolute() {
        return Err("Leyline can take a folder, a zip, or a direct https link.".into());
    }
    if path.is_dir() {
        return Ok(Source::Folder { path });
    }
    if path.is_file() {
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if matches!(ext.as_str(), "url" | "webloc") {
            // A link dragged out of a browser can arrive as a small shortcut file: the link inside it counts.
            let text = fs::read(&path).ok().filter(|b| b.len() < 64 * 1024).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
            let url = text.lines().find_map(|l| l.trim().strip_prefix("URL=").map(str::to_string)).or_else(|| {
                let i = text.find("<string>")? + 8;
                Some(text[i..].split("</string>").next()?.trim().to_string())
            });
            return match url {
                Some(u) if ["https://", "http://", "magnet:"].iter().any(|p| u.to_ascii_lowercase().starts_with(p)) => classify_with(&u, torrents),
                _ => Err("That shortcut has no link in it.".into()),
            };
        }
        if ext == "torrent" {
            if !torrents {
                return Err(TORRENT_REFUSAL.into());
            }
            return Ok(Source::Torrent { path });
        }
        if ext == "zip" || head(&path).starts_with(b"PK\x03\x04") {
            return Ok(Source::Zip { path });
        }
        if matches!(ext.as_str(), "7z" | "rar" | "tar" | "gz" | "xz" | "bz2" | "zst" | "iso") {
            return Err("Leyline can open zip files only. Unpack it yourself, then drop the folder.".into());
        }
        return Err("That file isn't a zip. Drop your game folder or a zip of it.".into());
    }
    Err("Leyline can't find that file or folder.".into())
}

fn head(p: &Path) -> Vec<u8> {
    let mut b = vec![0u8; 16];
    let n = fs::File::open(p).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
    b.truncate(n);
    b
}

// ------------------------------------------------------------------------------------------------ progress

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Downloading,
    Unpacking,
    Checking,
    Done,
    Failed,
    Cancelled,
}

impl Phase {
    pub fn busy(self) -> bool {
        matches!(self, Phase::Downloading | Phase::Unpacking | Phase::Checking)
    }
}

/// What the launcher's progress bar shows.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Progress {
    pub phase: Phase,
    /// Bytes downloaded or unpacked so far, and of how many (0 when not known).
    pub done: u64,
    pub total: u64,
    /// A plain sentence when it ended (done, failed or stopped).
    pub message: String,
    /// The client that was added, when done.
    pub client_id: Option<String>,
    pub client_name: Option<String>,
    /// Where it is on disk, when done.
    pub folder: Option<String>,
    /// Bytes per second over the last few seconds (0 when not known).
    pub speed: u64,
    /// The bar's one line while it runs: "Downloading · 1.2 GB / 5.0 GB · 14 MB/s · about 4 min left".
    pub line: String,
}

#[derive(Default)]
pub struct Job {
    cancel: AtomicBool,
    p: Mutex<Progress>,
    /// When the phase began, and the speed meter.
    m: Mutex<(Option<Instant>, crate::clienttorrent::Meter)>,
}

impl Job {
    pub fn snapshot(&self) -> Progress {
        let mut p = self.p.lock().map(|p| p.clone()).unwrap_or_default();
        if p.phase.busy() {
            p.speed = self.sample(p.done);
            p.line = crate::clienttorrent::line(p.phase, p.done, p.total, p.speed);
        }
        p
    }
    /// Note `done` now; the speed so far.
    fn sample(&self, done: u64) -> u64 {
        let Ok(mut m) = self.m.lock() else { return 0 };
        let t0 = *m.0.get_or_insert_with(Instant::now);
        m.1.push(t0.elapsed().as_millis() as u64, done);
        m.1.bps()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub(crate) fn phase(&self, phase: Phase, total: u64) {
        if let Ok(mut p) = self.p.lock() {
            p.phase = phase;
            p.done = 0;
            p.total = total;
        }
        if let Ok(mut m) = self.m.lock() {
            *m = (Some(Instant::now()), Default::default());
        }
    }
    fn add(&self, n: u64) {
        let done = match self.p.lock() {
            Ok(mut p) => {
                p.done += n;
                p.done
            }
            Err(_) => return,
        };
        self.sample(done);
    }
    /// The engine's own count (a torrent), and the size once it is known.
    pub(crate) fn set_done(&self, done: u64, total: u64) {
        if let Ok(mut p) = self.p.lock() {
            p.done = done;
            p.total = total;
        }
        self.sample(done);
    }
    /// Start a new import, unless one is running.
    pub fn begin(&self) -> Result<(), String> {
        let mut p = self.p.lock().map_err(|_| "import lock poisoned")?;
        if p.phase.busy() {
            return Err("Leyline is already adding a game client.".into());
        }
        *p = Progress { phase: Phase::Checking, ..Default::default() };
        self.cancel.store(false, Ordering::SeqCst);
        Ok(())
    }
    pub fn finish(&self, phase: Phase, message: String, added: Option<(&str, &str, &Path)>) {
        if let Ok(mut p) = self.p.lock() {
            p.phase = phase;
            p.message = message;
            if let Some((id, name, folder)) = added {
                p.client_id = Some(id.into());
                p.client_name = Some(name.into());
                p.folder = Some(folder.display().to_string());
            }
        }
    }
}

/// The one import the app runs at a time.
pub fn job() -> &'static Job {
    static J: OnceLock<Job> = OnceLock::new();
    J.get_or_init(Job::default)
}

// ------------------------------------------------------------------------------------------------ the steps

/// Download `url` into `dest` with progress and cancel, at most `max` bytes. Nothing is left at `dest` on failure.
pub fn download(url: &str, dest: &Path, max: u64, job: &Job) -> Result<u64, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(90))
        .user_agent(concat!("Leyline/", env!("CARGO_PKG_VERSION")))
        .build();
    let resp = agent.get(url).call().map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("The link answered {code}. Check it opens your zip in a browser."),
        e => format!("Couldn't reach the link ({e})."),
    })?;
    let total = resp.header("Content-Length").and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0);
    if total > max {
        return Err(format!("That download is bigger than {} GB, more than a game client.", max / GIB));
    }
    job.phase(Phase::Downloading, total);
    let res = (|| {
        if let Some(d) = dest.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut f = fs::File::create(dest).map_err(|e| format!("Couldn't save the download: {e}"))?;
        let mut r = resp.into_reader().take(max + 1);
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0u64;
        loop {
            if job.cancelled() {
                return Err(CANCELLED.to_string());
            }
            let n = r.read(&mut buf).map_err(|e| format!("The download broke off ({e}). Try again."))?;
            if n == 0 {
                break;
            }
            got += n as u64;
            if got > max {
                return Err(format!("That download is bigger than {} GB, more than a game client.", max / GIB));
            }
            f.write_all(&buf[..n]).map_err(|e| format!("Couldn't save the download: {e}"))?;
            job.add(n as u64);
        }
        if total > 0 && got != total {
            return Err("The download broke off. Try again.".into());
        }
        f.sync_all().map_err(|e| e.to_string())?;
        Ok(got)
    })();
    if res.is_err() {
        let _ = fs::remove_file(dest);
    }
    res
}

fn zip_err(e: zip::result::ZipError) -> String {
    match e {
        zip::result::ZipError::UnsupportedArchive(m) if m.to_ascii_lowercase().contains("password") => {
            "The zip has a password. Unpack it yourself, then drop the folder.".into()
        }
        _ => "The zip is damaged or packed in a way Leyline can't open. Unpack it yourself, then drop the folder.".into(),
    }
}

/// Unpack `zip` into `dest` (made here; it must not exist yet). Every entry is checked before anything is
/// written. On failure or cancel, `dest` is removed again.
pub fn unzip(zip: &Path, dest: &Path, caps: &Caps, job: &Job) -> Result<u64, String> {
    if dest.exists() {
        return Err("That folder is already there.".into());
    }
    let f = fs::File::open(zip).map_err(|e| format!("Couldn't open the zip: {e}"))?;
    let mut ar = zip::ZipArchive::new(io::BufReader::new(f)).map_err(|_| "That isn't a zip file, or it is damaged.".to_string())?;
    if ar.len() > caps.files {
        return Err(format!("The zip has more than {} files, more than a game client.", caps.files));
    }
    // 1. Check everything first.
    let mut plan: Vec<(usize, String, u64)> = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for i in 0..ar.len() {
        let file = ar.by_index_raw(i).map_err(zip_err)?;
        let raw = String::from_utf8_lossy(file.name_raw()).to_string();
        let rel = crate::mods::safe_rel(&raw).map_err(|_| format!("The zip has an unsafe path in it ({raw}). Nothing was unpacked."))?;
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(format!("The zip has a link in it ({raw}). Nothing was unpacked."));
        }
        let Some(rel) = rel else { continue }; // a folder entry
        if file.is_dir() {
            continue;
        }
        if file.encrypted() {
            return Err("The zip has a password. Unpack it yourself, then drop the folder.".into());
        }
        if !matches!(file.compression(), zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated) {
            return Err("The zip is packed in a way Leyline can't open. Unpack it yourself, then drop the folder.".into());
        }
        if !seen.insert(rel.to_ascii_lowercase()) {
            return Err(format!("The zip has {rel} twice. Nothing was unpacked."));
        }
        total = total.saturating_add(file.size());
        if total > caps.unpacked {
            return Err(format!("The zip unpacks to more than {} GB, more than a game client.", caps.unpacked / GIB));
        }
        plan.push((i, rel, file.size()));
    }
    // 2. Unpack.
    job.phase(Phase::Unpacking, total);
    fs::create_dir_all(dest).map_err(|e| format!("Couldn't make the folder {}: {e}", dest.display()))?;
    let res = (|| {
        let mut written = 0u64;
        let mut buf = vec![0u8; 256 * 1024];
        for (i, rel, size) in &plan {
            if job.cancelled() {
                return Err(CANCELLED.to_string());
            }
            let out = dest.join(rel);
            if !out.starts_with(dest) {
                return Err(format!("The zip has an unsafe path in it ({rel})."));
            }
            if let Some(d) = out.parent() {
                fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let mut w = fs::File::create(&out).map_err(|e| format!("Couldn't write {rel}: {e}"))?;
            let file = ar.by_index(*i).map_err(zip_err)?;
            let mut r = file.take(size + 1);
            let mut n_file = 0u64;
            loop {
                if job.cancelled() {
                    return Err(CANCELLED.to_string());
                }
                let n = r.read(&mut buf).map_err(|_| format!("The zip is damaged: {rel} doesn't unpack."))?;
                if n == 0 {
                    break;
                }
                n_file += n as u64;
                if n_file > *size {
                    return Err(format!("{rel} unpacks bigger than the zip says. Nothing was added."));
                }
                w.write_all(&buf[..n]).map_err(|e| format!("Couldn't write {rel}: {e}"))?;
                job.add(n as u64);
            }
            written += n_file;
        }
        Ok(written)
    })();
    if res.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    res
}

/// The client in `dir`: the folder itself, or a folder up to three levels inside (a zip often holds one folder).
pub fn find_client(dir: &Path) -> Option<(PathBuf, Detection)> {
    let mut level = vec![dir.to_path_buf()];
    for _ in 0..=3 {
        let mut next = Vec::new();
        for d in &level {
            if let Ok(det) = clients::detect(d) {
                return Some((d.clone(), det));
            }
            if let Ok(rd) = fs::read_dir(d) {
                let mut subs: Vec<PathBuf> = rd
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .map(|e| e.path())
                    .filter(|p| {
                        let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
                        !matches!(n.as_str(), "data" | "interface" | "wtf" | "cache" | "logs" | "screenshots" | "errors")
                    })
                    .collect();
                subs.sort();
                next.extend(subs);
            }
        }
        if next.len() > 400 {
            next.truncate(400);
        }
        level = next;
    }
    None
}

/// A plain folder name from a zip's or link's name.
pub fn folder_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let base = base.split(['?', '#']).next().unwrap_or("");
    let base = percent_decode(base);
    let stem = base.strip_suffix(".zip").or_else(|| base.strip_suffix(".ZIP")).unwrap_or(&base);
    let clean: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')') { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let clean = clean.trim_matches('.').trim().chars().take(60).collect::<String>();
    if clean.is_empty() { "Game client".into() } else { clean }
}

/// `parent/name`, or `parent/name (2)` and on, the first that is free.
pub fn free_folder(parent: &Path, name: &str) -> PathBuf {
    let first = parent.join(name);
    if !first.exists() {
        return first;
    }
    (2..).map(|n| parent.join(format!("{name} ({n})"))).find(|p| !p.exists()).unwrap()
}

const DOWNLOADED_NOT_A_CLIENT: &str = "It is downloaded, but no game client is in it. The right folder holds the game's program and a Data folder.";

/// A `.torrent`'s first bytes.
fn looks_like_torrent(h: &[u8]) -> bool {
    h.starts_with(b"d8:announce") || h.starts_with(b"d4:info") || h.starts_with(b"d13:announce") || h.starts_with(b"d7:comment") || h.starts_with(b"d10:created")
}

/// The biggest zip up to two folders inside `dir` (a torrent of a zipped client).
fn find_zip(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut level = vec![dir.to_path_buf()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for d in &level {
            for e in fs::read_dir(d).into_iter().flatten().flatten() {
                let Ok(t) = e.file_type() else { continue };
                let p = e.path();
                if t.is_dir() {
                    next.push(p);
                } else if t.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")) {
                    let len = e.metadata().map(|m| m.len()).unwrap_or(0);
                    if best.as_ref().map_or(true, |(b, _)| len > *b) {
                        best = Some((len, p));
                    }
                }
            }
        }
        next.truncate(400);
        level = next;
    }
    best.map(|(_, p)| p)
}

/// A torrent into a new folder under `parent`, then the client in it (or in the zip it held).
fn from_torrent(src: crate::clienttorrent::Src, name: &str, parent: &Path, caps: &Caps, job: &Job, share: bool) -> Result<(PathBuf, Detection), String> {
    let target = free_folder(parent, &folder_name(name));
    crate::clienttorrent::fetch(src, &target, caps, share, job)?;
    job.phase(Phase::Checking, 0);
    if let Some(f) = find_client(&target) {
        return Ok(f);
    }
    if let Some(z) = find_zip(&target) {
        let into = free_folder(parent, &folder_name(&z.to_string_lossy()));
        unzip(&z, &into, caps, job)?;
        job.phase(Phase::Checking, 0);
        if let Some(f) = find_client(&into) {
            let _ = fs::remove_file(&z); // the zip came from this download: the unpacked folder is what counts
            return Ok(f);
        }
        return Err(UNPACKED_NOT_A_CLIENT.into());
    }
    Err(DOWNLOADED_NOT_A_CLIENT.into())
}

/// A `.torrent`'s bytes: read, checked (every path, the caps), then downloaded.
fn from_torrent_file(raw: Vec<u8>, parent: &Path, caps: &Caps, job: &Job, share: bool) -> Result<(PathBuf, Detection), String> {
    let l = crate::clienttorrent::list_torrent(&raw)?;
    crate::clienttorrent::check_files(&l.files, caps)?;
    from_torrent(crate::clienttorrent::Src::File(raw), &l.name, parent, caps, job, share)
}

fn read_small(p: &Path, max: u64) -> Result<Vec<u8>, String> {
    let f = fs::File::open(p).map_err(|e| format!("Couldn't open {}: {e}", p.display()))?;
    let mut b = Vec::new();
    f.take(max + 1).read_to_end(&mut b).map_err(|e| e.to_string())?;
    if b.len() as u64 > max {
        return Err("That torrent file is too big.".into());
    }
    Ok(b)
}

/// The whole import: download (a link), unpack (a zip or link) into a new folder under `parent`, find the client.
/// `work` holds the download while it runs. Returns the client's folder and what detection found.
pub fn run(src: &Source, parent: &Path, work: &Path, caps: &Caps, job: &Job) -> Result<(PathBuf, Detection), String> {
    run_with(src, parent, work, caps, job, crate::clienttorrent::Testing::default())
}

/// `run`, with the testing switch as set: magnets and `.torrent`s only when it is on.
pub fn run_with(src: &Source, parent: &Path, work: &Path, caps: &Caps, job: &Job, t: crate::clienttorrent::Testing) -> Result<(PathBuf, Detection), String> {
    use crate::clienttorrent::{Src, MAX_TORRENT_BYTES};
    let torrent_ok = || if t.client_torrents { Ok(()) } else { Err(TORRENT_REFUSAL.to_string()) };
    let found = |dir: &Path| -> Result<(PathBuf, Detection), String> {
        job.phase(Phase::Checking, 0);
        find_client(dir).ok_or_else(|| NOT_A_CLIENT.to_string())
    };
    match src {
        Source::Magnet { uri, name } => {
            torrent_ok()?;
            from_torrent(Src::Magnet(uri.clone()), name.as_deref().unwrap_or("Game client"), parent, caps, job, t.share)
        }
        Source::Torrent { path } => {
            torrent_ok()?;
            from_torrent_file(read_small(path, MAX_TORRENT_BYTES)?, parent, caps, job, t.share)
        }
        Source::TorrentLink { url } => {
            torrent_ok()?;
            let tmp = work.join("downloads").join("game-client.torrent");
            download(url, &tmp, MAX_TORRENT_BYTES, job)?;
            let raw = read_small(&tmp, MAX_TORRENT_BYTES);
            let _ = fs::remove_file(&tmp);
            from_torrent_file(raw?, parent, caps, job, t.share)
        }
        Source::Folder { path } => found(path),
        Source::Zip { path } => {
            let target = free_folder(parent, &folder_name(&path.to_string_lossy()));
            unzip(path, &target, caps, job)?;
            found(&target).map_err(|_| UNPACKED_NOT_A_CLIENT.to_string())
        }
        Source::Link { url } => {
            let tmp = work.join("downloads").join("game-client.part");
            let _ = fs::remove_file(&tmp);
            download(url, &tmp, caps.download, job)?;
            let res = (|| {
                let h = head(&tmp);
                if looks_like_torrent(&h) {
                    // a link that answers with a .torrent (a share page or a worker that redirects to one)
                    torrent_ok()?;
                    let raw = read_small(&tmp, MAX_TORRENT_BYTES)?;
                    let _ = fs::remove_file(&tmp);
                    return from_torrent_file(raw, parent, caps, job, t.share);
                }
                if !h.starts_with(b"PK\x03\x04") {
                    return Err(if h.starts_with(b"7z\xbc\xaf") || h.starts_with(b"Rar!") {
                        "Leyline can open zip files only. Unpack it yourself, then drop the folder.".to_string()
                    } else {
                        "That link didn't give a zip file. Use a direct link to the zip itself.".to_string()
                    });
                }
                let target = free_folder(parent, &folder_name(url));
                unzip(&tmp, &target, caps, job)?;
                found(&target).map_err(|_| UNPACKED_NOT_A_CLIENT.to_string())
            })();
            let _ = fs::remove_file(&tmp);
            res
        }
    }
}

// ------------------------------------------------------------------------------------------------ the app

#[cfg(feature = "app")]
pub(crate) mod commands {
    use super::*;
    use tauri::{AppHandle, Manager, State};
    use tauri_plugin_dialog::DialogExt;

    type Core<'a> = State<'a, crate::app::Core>;

    #[derive(Serialize)]
    pub struct Checked {
        #[serde(flatten)]
        source: Source,
        name: String,
        unpacks: bool,
        /// Where a zip goes unless the player picks another folder.
        default_folder: String,
    }

    #[tauri::command]
    pub fn client_import_check(core: Core, input: String) -> Result<Checked, String> {
        let t = crate::clienttorrent::load_testing(&core.dir);
        let source = classify_with(&input, t.client_torrents)?;
        Ok(Checked {
            name: source.name(),
            unpacks: source.unpacks(),
            default_folder: core.dir.join(DEFAULT_FOLDER).display().to_string(),
            source,
        })
    }

    #[tauri::command]
    pub fn client_import_start(app: AppHandle, input: String, folder: Option<String>) -> Result<(), String> {
        let core = app.state::<crate::app::Core>();
        let t = crate::clienttorrent::load_testing(&core.dir);
        let source = classify_with(&input, t.client_torrents)?;
        let parent = match folder.map(|f| f.trim().to_string()).filter(|f| !f.is_empty()) {
            Some(f) => PathBuf::from(f),
            None => core.dir.join(DEFAULT_FOLDER),
        };
        if source.unpacks() {
            if !parent.is_absolute() {
                return Err("Pick a folder to unpack into.".into());
            }
            fs::create_dir_all(&parent).map_err(|e| format!("Couldn't use the folder {}: {e}", parent.display()))?;
        }
        job().begin()?;
        let work = core.dir.clone();
        std::thread::spawn(move || {
            let j = job();
            match run_with(&source, &parent, &work, &CAPS, j, t) {
                Ok((path, det)) => {
                    let core = app.state::<crate::app::Core>();
                    let added = core.update(|s| {
                        if let Some(c) = s.clients.iter().find(|c| c.path == path) {
                            let c = c.clone();
                            s.selected_client = Some(c.id.clone());
                            return Ok((c, false));
                        }
                        let p = clients::new_profile(&path, &det, &s.clients);
                        s.clients.push(p.clone());
                        s.selected_client = Some(p.id.clone());
                        Ok((p, true))
                    });
                    match added {
                        Ok((p, new)) => {
                            let msg = if !new {
                                format!("{} is already in your game clients.", p.name)
                            } else if det.complete {
                                format!("Added {}.", p.name)
                            } else {
                                format!("Added {}. Some files are missing: {}", p.name, det.missing.join(", "))
                            };
                            j.finish(Phase::Done, msg, Some((&p.id, &p.name, &path)));
                        }
                        Err(e) => j.finish(Phase::Failed, e, None),
                    }
                }
                Err(e) if e == CANCELLED => j.finish(Phase::Cancelled, e, None),
                Err(e) => j.finish(Phase::Failed, e, None),
            }
        });
        Ok(())
    }

    #[tauri::command]
    pub fn client_import_state() -> Progress {
        job().snapshot()
    }

    #[tauri::command]
    pub fn client_import_cancel() {
        job().cancel();
    }

    async fn pick(app: AppHandle, title: &'static str, zip: bool) -> Option<String> {
        tauri::async_runtime::spawn_blocking(move || {
            let d = app.dialog().file().set_title(title);
            if zip { d.add_filter("Zip", &["zip", "ZIP"]).blocking_pick_file() } else { d.blocking_pick_folder() }
        })
        .await
        .ok()
        .flatten()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
    }

    #[tauri::command]
    pub async fn client_import_pick_zip(app: AppHandle) -> Option<String> {
        pick(app, "Choose the zip of your game client", true).await
    }

    #[tauri::command]
    pub async fn client_import_pick_folder(app: AppHandle) -> Option<String> {
        pick(app, "Choose where to unpack your game client", false).await
    }

    /// The play bar's drop: "Where should the game go?", opening at `start` (the last choice) or the data dir's
    /// Game clients folder, which is made first so the picker can open in it.
    #[tauri::command]
    pub async fn client_import_pick_dest(app: AppHandle, start: Option<String>) -> Option<String> {
        let dflt = app.state::<crate::app::Core>().dir.join(DEFAULT_FOLDER);
        let at = start.map(PathBuf::from).filter(|p| p.is_absolute() && p.is_dir()).unwrap_or_else(|| {
            let _ = fs::create_dir_all(&dflt);
            dflt
        });
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().set_title("Where should the game go?").set_directory(at).blocking_pick_folder())
            .await
            .ok()
            .flatten()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.display().to_string())
    }

    /// Settings › Testing (on this PC only; off by default).
    #[tauri::command]
    pub fn testing_get(core: Core) -> crate::clienttorrent::Testing {
        crate::clienttorrent::load_testing(&core.dir)
    }

    #[tauri::command]
    pub fn testing_set(core: Core, client_torrents: bool, share: bool) -> Result<crate::clienttorrent::Testing, String> {
        crate::clienttorrent::save_testing(&core.dir, crate::clienttorrent::Testing { client_torrents, share })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::testutil::fake_335a;
    use crate::net::testserver::serve_routes;
    use std::io::Cursor;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            if name.ends_with('/') {
                w.add_directory(name.trim_end_matches('/'), o).unwrap();
            } else {
                w.start_file(*name, o).unwrap();
                w.write_all(data).unwrap();
            }
        }
        w.finish().unwrap().into_inner()
    }

    /// A zip of a fake 3.3.5a client (placeholder files, no game data) inside one top folder.
    fn client_zip(top: &str) -> Vec<u8> {
        let src = tempfile::tempdir().unwrap();
        fake_335a(src.path(), true);
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut stack = vec![src.path().to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    let rel = e.path().strip_prefix(src.path()).unwrap().to_string_lossy().replace('\\', "/");
                    entries.push((format!("{top}/{rel}"), fs::read(e.path()).unwrap()));
                }
            }
        }
        let refs: Vec<(&str, &[u8])> = entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())).collect();
        zip_of(&refs)
    }

    #[test]
    fn says_what_was_dropped() {
        let d = tempfile::tempdir().unwrap();
        let z = d.path().join("My Client.zip");
        fs::write(&z, zip_of(&[("a.txt", b"x")])).unwrap();
        let renamed = d.path().join("backup.bin");
        fs::write(&renamed, zip_of(&[("a.txt", b"x")])).unwrap();
        let seven = d.path().join("c.7z");
        fs::write(&seven, b"7z\xbc\xaf\x27\x1c").unwrap();
        let txt = d.path().join("notes.txt");
        fs::write(&txt, b"hi").unwrap();

        assert_eq!(classify(&d.path().display().to_string()).unwrap(), Source::Folder { path: d.path().to_path_buf() });
        assert_eq!(classify(&z.display().to_string()).unwrap(), Source::Zip { path: z.clone() });
        assert_eq!(classify(&renamed.display().to_string()).unwrap(), Source::Zip { path: renamed.clone() }, "by its first bytes");
        assert!(classify(&seven.display().to_string()).unwrap_err().contains("zip files only"));
        assert!(classify(&txt.display().to_string()).unwrap_err().contains("isn't a zip"));
        // an absolute path that isn't there ("/no/such/place" isn't absolute on Windows: no drive letter)
        let missing = if cfg!(windows) { "C:\\no\\such\\place" } else { "/no/such/place" };
        assert!(classify(missing).unwrap_err().contains("can't find"));
        // a link dragged out of a browser as a shortcut file
        let url = d.path().join("backup.url");
        fs::write(&url, "[InternetShortcut]\r\nURL=https://files.example.com/mine.zip\r\n").unwrap();
        assert_eq!(classify(&url.display().to_string()).unwrap(), Source::Link { url: "https://files.example.com/mine.zip".into() });
        let wl = d.path().join("backup.webloc");
        fs::write(&wl, "<plist><dict><key>URL</key><string>magnet:?xt=urn:btih:abc</string></dict></plist>").unwrap();
        assert_eq!(classify(&wl.display().to_string()).unwrap_err(), TORRENT_REFUSAL);
        let looped = d.path().join("loop.url");
        fs::write(&looped, format!("URL=file://{}", looped.display())).unwrap();
        assert!(classify(&looped.display().to_string()).is_err(), "a shortcut never points at another file");
        assert!(classify("relative/path").is_err());
        assert!(classify("   ").is_err());
        // a file:// address, with %20
        let u = format!("file://{}", z.display().to_string().replace(' ', "%20"));
        if cfg!(unix) {
            assert_eq!(classify(&u).unwrap(), Source::Zip { path: z.clone() });
        }
        // links: https only, plain hosts, the first line of a uri-list
        assert_eq!(classify("https://files.example.com/my-backup.zip").unwrap(), Source::Link { url: "https://files.example.com/my-backup.zip".into() });
        assert_eq!(classify("# a comment\r\nhttps://files.example.com/b.zip\r\n").unwrap(), Source::Link { url: "https://files.example.com/b.zip".into() });
        assert!(classify("http://files.example.com/b.zip").unwrap_err().contains("starts with https"));
        assert!(classify("https://user:pw@files.example.com/b.zip").is_err());
        assert!(classify("ftp://files.example.com/b.zip").is_err());
        assert_eq!(Source::Link { url: "https://files.example.com/b.zip".into() }.name(), "files.example.com");
    }

    #[test]
    fn refuses_torrents_and_magnets_in_plain_words() {
        for t in [
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=client",
            "MAGNET:?xt=urn:btih:abc",
            "https://files.example.com/client.torrent",
            "https://files.example.com/client.TORRENT?x=1",
            "/home/me/Downloads/client.torrent",
        ] {
            assert_eq!(classify(t).unwrap_err(), TORRENT_REFUSAL, "{t}");
        }
        assert_eq!(TORRENT_REFUSAL, "Leyline can't download game clients from torrents. Drop your own client folder or zip instead.");
    }

    #[test]
    fn the_testing_switch_takes_magnets_and_torrents_and_off_refuses_them() {
        let d = tempfile::tempdir().unwrap();
        let tf = d.path().join("client.torrent");
        fs::write(&tf, crate::p2p::torrent::make("c.zip", b"PK\x03\x04", 16 * 1024, &[])).unwrap();
        let mag = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=My%20Client&tr=udp%3A%2F%2Ft.example%3A1";
        // off: today's one sentence, for every kind
        for t in [mag.to_string(), "https://files.example.com/c.torrent".into(), tf.display().to_string()] {
            assert_eq!(classify_with(&t, false).unwrap_err(), TORRENT_REFUSAL, "{t}");
        }
        // on
        assert_eq!(classify_with(mag, true).unwrap(), Source::Magnet { uri: mag.into(), name: Some("My Client".into()) });
        assert_eq!(classify_with(mag, true).unwrap().name(), "My Client");
        assert_eq!(classify_with("https://pub-abc.r2.dev/c.torrent?x=1", true).unwrap(), Source::TorrentLink { url: "https://pub-abc.r2.dev/c.torrent?x=1".into() });
        assert_eq!(classify_with(&tf.display().to_string(), true).unwrap(), Source::Torrent { path: tf.clone() });
        assert!(classify_with(&tf.display().to_string(), true).unwrap().unpacks());
        assert!(classify_with("magnet:?dn=nohash", true).unwrap_err().contains("doesn't look right"));
        assert!(classify_with("http://files.example.com/c.torrent", true).unwrap_err().contains("https"));
        assert!(classify_with("/no/such/c.torrent", true).is_err());
        // Cloudflare-style direct links are plain links
        for u in ["https://pub-0123abcd.r2.dev/backup.zip", "https://my-files.pages.dev/b.zip", "https://dl.me.workers.dev/get?id=1"] {
            assert_eq!(classify_with(u, true).unwrap(), Source::Link { url: u.into() });
        }
        // run: the switch is checked again, whatever was classified
        let off = crate::clienttorrent::Testing::default();
        let e = run_with(&Source::Magnet { uri: mag.into(), name: None }, d.path(), d.path(), &CAPS, &Job::default(), off).unwrap_err();
        assert_eq!(e, TORRENT_REFUSAL);
        let e = run_with(&Source::Torrent { path: tf.clone() }, d.path(), d.path(), &CAPS, &Job::default(), off).unwrap_err();
        assert_eq!(e, TORRENT_REFUSAL);
        // a link that answers with a .torrent: refused with the switch off
        let base = serve_routes(vec![("/get", "200 OK", fs::read(&tf).unwrap())]);
        let e = run_with(&Source::Link { url: format!("{base}/get") }, d.path(), d.path(), &CAPS, &Job::default(), off).unwrap_err();
        assert_eq!(e, TORRENT_REFUSAL);
        // a torrent with an unsafe path is refused before the engine starts, with the switch on
        let on = crate::clienttorrent::Testing { client_torrents: true, share: false };
        let bad = d.path().join("bad.torrent");
        fs::write(&bad, crate::clienttorrent::tests::make_multi("x", &[(&["..", "..", "evil"], b"1")], 16 * 1024)).unwrap();
        let e = run_with(&Source::Torrent { path: bad }, &d.path().join("c"), d.path(), &CAPS, &Job::default(), on).unwrap_err();
        assert!(e.contains("unsafe path"), "{e}");
        assert!(!d.path().join("c").join("x").exists());
    }

    #[test]
    fn the_bar_line_follows_the_job() {
        let job = Job::default();
        job.phase(Phase::Downloading, 5 * GIB);
        job.set_done(1_288_490_189, 5 * GIB);
        let p = job.snapshot();
        assert!(p.line.starts_with("Downloading · 1.2 GB / 5.0 GB"), "{}", p.line);
        job.phase(Phase::Checking, 0);
        assert_eq!(job.snapshot().line, "Checking your game folder…");
        job.finish(Phase::Done, "Added X.".into(), None);
        assert_eq!(job.snapshot().line, "", "the message says the end");
    }

    #[test]
    fn unzips_and_finds_the_client_inside() {
        let d = tempfile::tempdir().unwrap();
        let z = d.path().join("Wrath backup.zip");
        fs::write(&z, client_zip("Game")).unwrap();
        let parent = d.path().join("clients");
        let job = Job::default();
        let (root, det) = run(&Source::Zip { path: z.clone() }, &parent, d.path(), &CAPS, &job).unwrap();
        assert_eq!(root, parent.join("Wrath backup").join("Game"), "one level inside the unpacked folder");
        assert_eq!(det.build, clients::Build::V335a);
        assert!(det.complete);
        let p = job.snapshot();
        assert_eq!(p.phase, Phase::Checking);
        // a second import of the same zip goes next to it
        let (root2, _) = run(&Source::Zip { path: z }, &parent, d.path(), &CAPS, &Job::default()).unwrap();
        assert_eq!(root2, parent.join("Wrath backup (2)").join("Game"));
    }

    #[test]
    fn a_dropped_folder_is_added_where_it_is() {
        let d = tempfile::tempdir().unwrap();
        fake_335a(&d.path().join("Games").join("Wrath"), false);
        let (root, det) = run(&Source::Folder { path: d.path().join("Games") }, d.path(), d.path(), &CAPS, &Job::default()).unwrap();
        assert_eq!(root, d.path().join("Games").join("Wrath"));
        assert_eq!(det.build, clients::Build::V335a);
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(run(&Source::Folder { path: empty.path().into() }, d.path(), d.path(), &CAPS, &Job::default()).unwrap_err(), NOT_A_CLIENT);
    }

    #[test]
    fn a_zip_without_a_client_says_so_and_keeps_the_files() {
        let d = tempfile::tempdir().unwrap();
        let z = d.path().join("photos.zip");
        fs::write(&z, zip_of(&[("pics/a.txt", b"x")])).unwrap();
        let e = run(&Source::Zip { path: z }, &d.path().join("c"), d.path(), &CAPS, &Job::default()).unwrap_err();
        assert_eq!(e, UNPACKED_NOT_A_CLIENT);
        assert!(d.path().join("c").join("photos").join("pics").join("a.txt").is_file());
    }

    #[test]
    fn zip_slip_and_links_are_refused_before_anything_is_written() {
        let d = tempfile::tempdir().unwrap();
        for (i, bad) in ["../evil.txt", "a/../../evil.txt", "/abs.txt", "C:/win.txt", "a\\..\\..\\evil.txt"].iter().enumerate() {
            let z = d.path().join(format!("bad{i}.zip"));
            fs::write(&z, zip_of(&[("ok.txt", b"fine"), (bad, b"x")])).unwrap();
            let dest = d.path().join(format!("out{i}"));
            let e = unzip(&z, &dest, &CAPS, &Job::default()).unwrap_err();
            assert!(e.contains("unsafe path"), "{bad}: {e}");
            assert!(!dest.exists(), "{bad}: nothing written");
        }
        assert!(!d.path().join("evil.txt").exists() && !d.path().parent().unwrap().join("evil.txt").exists());
        // a symbolic link entry
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        w.add_symlink("link", "/etc/passwd", zip::write::SimpleFileOptions::default()).unwrap();
        let z = d.path().join("link.zip");
        fs::write(&z, w.finish().unwrap().into_inner()).unwrap();
        assert!(unzip(&z, &d.path().join("outl"), &CAPS, &Job::default()).unwrap_err().contains("link"));
        // two names that differ only in case
        let z = d.path().join("twice.zip");
        fs::write(&z, zip_of(&[("Data/a.txt", b"1"), ("data/A.txt", b"2")])).unwrap();
        assert!(unzip(&z, &d.path().join("outt"), &CAPS, &Job::default()).unwrap_err().contains("twice"));
        // a folder that is already there is never unpacked into
        fs::create_dir_all(d.path().join("there")).unwrap();
        assert!(unzip(&d.path().join("twice.zip"), &d.path().join("there"), &CAPS, &Job::default()).unwrap_err().contains("already there"));
    }

    #[test]
    fn size_and_file_caps() {
        let d = tempfile::tempdir().unwrap();
        let big = vec![0u8; 300_000];
        let z = d.path().join("big.zip");
        fs::write(&z, zip_of(&[("a.bin", &big), ("b.bin", &big), ("c.bin", &big), ("d.bin", &big)])).unwrap();
        let caps = Caps { download: 1 << 30, unpacked: 1_000_000, files: 100 };
        assert!(unzip(&z, &d.path().join("o1"), &caps, &Job::default()).unwrap_err().contains("more than"));
        assert!(!d.path().join("o1").exists());
        let many: Vec<(String, Vec<u8>)> = (0..12).map(|i| (format!("f{i}.txt"), vec![1u8])).collect();
        let refs: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let z = d.path().join("many.zip");
        fs::write(&z, zip_of(&refs)).unwrap();
        let caps = Caps { download: 1 << 30, unpacked: 1 << 30, files: 10 };
        assert!(unzip(&z, &d.path().join("o2"), &caps, &Job::default()).unwrap_err().contains("more than 10 files"));
        // progress adds up to the unpacked size
        let job = Job::default();
        assert_eq!(unzip(&d.path().join("big.zip"), &d.path().join("o3"), &CAPS, &job).unwrap(), 1_200_000);
        let p = job.snapshot();
        assert_eq!((p.phase, p.done, p.total), (Phase::Unpacking, 1_200_000, 1_200_000));
    }

    #[test]
    fn downloads_a_link_with_progress_then_unpacks_it() {
        let d = tempfile::tempdir().unwrap();
        let body: &'static [u8] = Box::leak(client_zip("Wrath").into_boxed_slice());
        let base = serve_routes(vec![
            ("/my%20backup.zip", "200 OK", body.to_vec()),
            ("/page.zip", "200 OK", b"<html>not a zip</html>".to_vec()),
            ("/gone.zip", "404 Not Found", b"no".to_vec()),
        ]);
        let job = Job::default();
        let src = Source::Link { url: format!("{base}/my%20backup.zip") };
        let (root, det) = run(&src, &d.path().join("c"), d.path(), &CAPS, &job).unwrap();
        assert_eq!(root, d.path().join("c").join("my backup").join("Wrath"));
        assert_eq!(det.build, clients::Build::V335a);
        assert!(!d.path().join("downloads").join("game-client.part").exists(), "the download is removed after unpacking");
        let e = run(&Source::Link { url: format!("{base}/page.zip") }, &d.path().join("c"), d.path(), &CAPS, &Job::default()).unwrap_err();
        assert!(e.contains("didn't give a zip"), "{e}");
        let e = run(&Source::Link { url: format!("{base}/gone.zip") }, &d.path().join("c"), d.path(), &CAPS, &Job::default()).unwrap_err();
        assert!(e.contains("404"), "{e}");
        // the download cap, by its length
        let e = download(&format!("{base}/my%20backup.zip"), &d.path().join("x.part"), 100, &Job::default()).unwrap_err();
        assert!(e.contains("bigger than"), "{e}");
        assert!(!d.path().join("x.part").exists());
    }

    #[test]
    fn cancel_stops_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        let z = d.path().join("c.zip");
        fs::write(&z, client_zip("Wrath")).unwrap();
        let job = Job::default();
        job.cancel();
        assert_eq!(unzip(&z, &d.path().join("out"), &CAPS, &job).unwrap_err(), CANCELLED);
        assert!(!d.path().join("out").exists(), "the half-unpacked folder is removed");
        let base = serve_routes(vec![("/c.zip", "200 OK", fs::read(&z).unwrap())]);
        assert_eq!(download(&format!("{base}/c.zip"), &d.path().join("dl.part"), CAPS.download, &job).unwrap_err(), CANCELLED);
        assert!(!d.path().join("dl.part").exists());
        // a new import starts fresh; only one at a time
        let j = Job::default();
        j.cancel();
        j.begin().unwrap();
        assert!(!j.cancelled());
        assert!(j.begin().unwrap_err().contains("already"));
        j.finish(Phase::Cancelled, CANCELLED.into(), None);
        j.begin().unwrap();
    }

    #[test]
    fn folder_names() {
        assert_eq!(folder_name("/x/My Wrath.zip"), "My Wrath");
        assert_eq!(folder_name("https://files.example.com/a/b/my%20backup.zip?dl=1"), "my backup");
        assert_eq!(folder_name("https://files.example.com/"), "Game client");
        assert_eq!(folder_name("C:\\Users\\me\\..zip"), "Game client");
        assert_eq!(folder_name("weird<>:name.zip"), "weird name");
        let d = tempfile::tempdir().unwrap();
        assert_eq!(free_folder(d.path(), "A"), d.path().join("A"));
        fs::create_dir(d.path().join("A")).unwrap();
        assert_eq!(free_folder(d.path(), "A"), d.path().join("A (2)"));
    }
}
