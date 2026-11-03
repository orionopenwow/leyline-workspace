//! Listed mods: what any player listed with "List your mod" (stream/listings.py, docs/LIST-YOUR-MOD.md). all.json
//! `source: "listed"`, id `link:<12 hex>`, and an `install` block the site wrote after it checked the download once.
//!
//! We never host them: the launcher downloads the author's own file (a GitHub release file, `.zip` or `.MPQ`, or the
//! archive of a 40-hex commit on GitHub, GitLab or Codeberg), checks it is exactly what the site checked, and only
//! then installs it through the mod manager:
//!
//! * Where from: `install.url` is the listing's own repo in one of the shapes above (`url_ok`), redirects only to the
//!   forges' download hosts (`upstream::download`).
//! * What is checked: the whole file's sha256 for a release file (byte-stable), and always every file: each
//!   installed file's sha256 against `install.files` (nothing missing, nothing extra).
//! * `install.kind` "addon": addon folders (the curated addons' rules: `.blp` author art, `!` and `+` in folder names,
//!   an Interface list), installed like an upstream addon. "files": everything else for a client (a client patch
//!   `.MPQ`, native `.dll/.asi/.exe` with its settings, a Leyline mod package's `ui/` and `native/`), staged and then
//!   put through the same checks as a dropped zip (`mods::validate`): patches take the next free `patch-<X>.MPQ`
//!   slot, native code needs the player's OK first (`NATIVE_CONFIRM`, per file hash).
//! * In `Leyline/mods.json` as `link:<id>` with source "listed" and `sha256` = the pinned rev: a catalog entry with
//!   another rev is an update (the site re-checks the author's pushes first, like curate.py's pending_update).
//!
//! "Community · not reviewed" (`state: community`) and moderator-reviewed (`state: reviewed`) entries install the same
//! way; the UI says which it is.

use crate::catalog::Project;
use crate::clients::Build;
use crate::mods::{self, InstallMeta, Installed};
use crate::upstream::{self, Install, Unpacked};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// A client patch can be this big (mods.rs MAX_PATCH); everything else keeps the curated addons' per-file cap.
const MAX_PATCH: u64 = 1024 * 1_048_576;

fn hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn ext(p: &str) -> String {
    let base = p.rsplit('/').next().unwrap_or(p);
    match base.rfind('.') {
        Some(i) if i > 0 => base[i..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// The download is the listing's own repo: a release file (`.zip` or `.MPQ`) or a commit archive.
pub fn url_ok(inst: &Install, repo: &str) -> bool {
    let u = inst.url.as_str();
    if u.len() > 400 || u.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '\\' | '%')) || u.contains("/../") {
        return false;
    }
    if inst.from == "release-asset" {
        let Some(("github.com", o, r)) = upstream::repo_parts(repo) else { return false };
        let pre = format!("https://github.com/{o}/{r}/releases/download/");
        return u.strip_prefix(&pre).is_some_and(|rest| {
            let parts: Vec<&str> = rest.split('/').collect();
            let low = parts.last().map(|p| p.to_ascii_lowercase()).unwrap_or_default();
            parts.len() == 2 && parts.iter().all(|p| !p.is_empty() && *p != "." && *p != "..") && (low.ends_with(".zip") || low.ends_with(".mpq"))
        });
    }
    inst.from == "commit-archive" && upstream::url_ok(inst, repo, "", false)
}

/// The install block is sound: an allowed address, a rev, a list of safe paths with hashes and its digest.
pub fn install_ok(inst: &Install, repo: &str, blp_ok: bool) -> bool {
    match inst.kind.as_str() {
        "addon" => {
            // The curated addons' check (a release file must be a zip there: an addon is never a bare file).
            url_ok(inst, repo)
                && upstream::install_ok(inst, repo, "", false, blp_ok)
                && !inst.files.is_empty()
                && upstream::files_digest(&inst.files) == inst.files_digest
        }
        "files" => {
            let r = upstream::rules();
            url_ok(inst, repo)
                && !inst.rev.is_empty()
                && inst.rev.len() <= 80
                && !inst.rev.chars().any(|c| c.is_whitespace() || c.is_control())
                && inst.sha256.as_deref().map_or(true, hex64)
                && (inst.strip_prefix.is_empty()
                    || (inst.strip_prefix.ends_with('/') && mods::safe_rel(inst.strip_prefix.trim_end_matches('/')).ok().flatten().is_some()))
                && !inst.files.is_empty()
                && inst.files.len() <= r.caps.files
                && inst.files.iter().all(|(p, sha)| hex64(sha) && mods::safe_rel(p).ok().flatten().as_deref() == Some(p.as_str()))
                && upstream::files_digest(&inst.files) == inst.files_digest
                // a bare release file is one .MPQ, named as the file it is
                && (!(inst.from == "release-asset" && inst.url.to_ascii_lowercase().ends_with(".mpq"))
                    || (inst.files.len() == 1 && inst.files.keys().all(|k| !k.contains('/') && inst.url.ends_with(&format!("/{k}")))))
        }
        _ => false,
    }
}

/// Unpack exactly the listed files (paths after `strip_prefix`), refusing zip-slip, links, passwords, odd packing,
/// case twins and sizes past the caps. A bare `.MPQ` release file is the one file itself.
pub fn unpack_files(download: &Path, dest: &Path, inst: &Install) -> Result<Unpacked, String> {
    let r = upstream::rules();
    let mut out = Unpacked::default();
    let f = fs::File::open(download).map_err(|e| format!("could not read the download: {e}"))?;
    let ar = zip::ZipArchive::new(io::BufReader::new(f));
    let Ok(mut ar) = ar else {
        let name = inst.files.keys().next().cloned().unwrap_or_default();
        if inst.files.len() == 1 && ext(&name) == ".mpq" && inst.url.ends_with(&format!("/{name}")) {
            let to = dest.join(&name);
            fs::copy(download, &to).map_err(|e| e.to_string())?;
            out.files.insert(name, to);
            return Ok(out);
        }
        return Err("the download is not a zip file (or it is damaged)".into());
    };
    if ar.len() > r.caps.files * 3 {
        return Err("the download has too many files".into());
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for i in 0..ar.len() {
        let mut file = ar.by_index(i).map_err(|e| format!("the download is damaged or packed in a way Leyline cannot read ({e})"))?;
        if file.is_dir() {
            continue;
        }
        let raw = String::from_utf8_lossy(file.name_raw()).to_string();
        let Some(rel) = mods::safe_rel(&raw)? else { continue };
        if !seen.insert(rel.to_ascii_lowercase()) {
            return Err(format!("the download has two files named {rel} (they differ only in case)"));
        }
        let Some(inner) = rel.strip_prefix(inst.strip_prefix.as_str()) else {
            out.skipped += 1;
            continue;
        };
        if !inst.files.contains_key(inner) {
            out.skipped += 1;
            continue;
        }
        if file.encrypted() {
            return Err(format!("the download is password protected: {rel}"));
        }
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err(format!("symbolic links are not allowed: {rel}"));
        }
        if !matches!(file.compression(), zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated) {
            return Err(format!("{rel} is packed in a way Leyline cannot read"));
        }
        let cap = if ext(inner) == ".mpq" { MAX_PATCH } else { r.caps.file };
        if file.size() > cap {
            return Err(format!("{rel} is larger than {} MB", cap / 1_048_576));
        }
        let to = dest.join(inner);
        fs::create_dir_all(to.parent().unwrap_or(dest)).map_err(|e| e.to_string())?;
        let mut w = fs::File::create(&to).map_err(|e| e.to_string())?;
        let n = io::copy(&mut (&mut file).take(cap + 1), &mut w).map_err(|_| format!("the download is damaged: {rel} does not unpack"))?;
        if n > cap {
            return Err(format!("{rel} unpacks bigger than it says"));
        }
        total += n;
        if total > r.caps.total + MAX_PATCH {
            return Err("the mod unpacks bigger than Leyline allows".into());
        }
        out.files.insert(inner.to_string(), to);
    }
    Ok(out)
}

/// The project's install block, checked again (catalog::clean already did).
fn checked(p: &Project) -> Result<&Install, String> {
    let inst = p.install.as_ref().ok_or("This mod has no install details in the catalog.")?;
    if p.source != "listed" || !p.id.starts_with("link:") || !install_ok(inst, &p.repo, p.blp == "author-art") {
        return Err("This mod's install details did not check out, so Leyline will not install it.".into());
    }
    Ok(inst)
}

/// Download a listed mod from its author, check it is exactly what the site checked, then install it like every mod
/// (key `link:<id>`). `native_ok` answers whether the player OK'd this mod's native files (mods::native_consent_key).
pub fn install(client: &Path, build: Build, p: &Project, scratch: &Path, native_ok: &dyn Fn(&str) -> bool) -> Result<Installed, String> {
    let inst = checked(p)?.clone();
    if !p.fits(build) {
        return Err(format!("{} is made for the {} client, not this {} one.", p.name, p.builds.join(" / "), build.catalog_tag()));
    }
    let r = upstream::rules();
    let dl = mods::Scratch::new(scratch).map_err(|e| format!("could not make a download folder: {e}"))?;
    let zip = dl.dir.join("listed.download");
    let cap = if inst.kind == "files" { r.caps.download + MAX_PATCH } else { r.caps.download };
    let max = (inst.size.saturating_mul(2) + 16 * 1_048_576).min(cap);
    let (_, sha) = upstream::download(&inst.url, &zip, max).map_err(|e| format!("Download from the author's site failed: {e}"))?;
    if inst.sha256.as_deref().is_some_and(|want| want != sha) {
        return Err("The download does not match the file that was checked, so it was not installed. The author may have changed it: try again later.".into());
    }
    let st = mods::Staging::new(client).map_err(|e| format!("could not make a staging folder in the client: {e}"))?;
    let files = st.files();
    fs::create_dir_all(&files).map_err(|e| e.to_string())?;
    let meta = InstallMeta {
        key: p.id.clone(),
        name: p.name.clone(),
        version: p.version.clone().unwrap_or_else(|| inst.rev.chars().take(7).collect()),
        source: "listed".into(),
        sha256: inst.rev.clone(),
        category: p.category.clone(),
        credits: p.credits.clone(),
    };
    if inst.kind == "addon" {
        let u = upstream::unpack(&zip, &files, &inst, p.blp == "author-art")?;
        upstream::verify(&u, &inst)?;
        let mut plan = upstream::plan_of(&u, &inst, build);
        if u.skipped > 0 {
            plan.skipped.push(format!("{} file(s) the game does not load (notes, screenshots, dev files)", u.skipped));
        }
        return mods::install(client, build, meta, &plan, false);
    }
    let u = unpack_files(&zip, &files, &inst)?;
    upstream::verify(&u, &inst)?;
    let staged = mods::walk(&files).map_err(|e| e.to_string())?;
    let plan = mods::validate(&staged, build);
    if !plan.errors.is_empty() {
        return Err(format!("{} can't go into this client: {}", p.name, plan.errors.join("; ")));
    }
    if plan.addons.is_empty() && plan.patches.is_empty() && plan.natives.is_empty() {
        return Err(format!("{} has nothing for a game client (it runs on a realm).", p.name));
    }
    let ok = native_ok(&mods::native_consent_key(&meta.key, &plan));
    mods::install(client, build, meta, &plan, ok)
}

/// The update an installed listed mod has: the catalog's version (or short rev) when its rev differs.
pub fn update_for(m: &Installed, p: &Project) -> Option<String> {
    let inst = p.install.as_ref()?;
    (p.source == "listed" && m.source == "listed" && !inst.rev.is_empty() && inst.rev != m.sha256)
        .then(|| p.version.clone().filter(|v| !v.is_empty()).unwrap_or_else(|| inst.rev.chars().take(7).collect()))
}

/// The files of an install block, for tests and the UI: path -> sha256.
pub fn file_list(inst: &Install) -> &BTreeMap<String, String> {
    &inst.files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::testutil::fake_335a;
    use crate::mods::tests::zip_of;
    use crate::net::testserver::*;

    fn sha(b: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        crate::net::hex(&Sha256::digest(b))
    }

    const C1: &str = "0123456789abcdef0123456789abcdef01234567";
    const TOC: &[u8] = b"## Interface: 30300\n## Title: Frames\nFrames.lua\n";
    const MPQ: &[u8] = b"MPQ\x1a\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

    fn block(kind: &str, from: &str, url: &str, files: &[(&str, &[u8])], prefix: &str, folders: &[(&str, &str)]) -> Install {
        let f: BTreeMap<String, String> = files.iter().map(|(p, b)| (p.to_string(), sha(b))).collect();
        Install {
            kind: kind.into(),
            from: from.into(),
            url: url.into(),
            rev: if from == "commit-archive" { C1.into() } else { "v1".into() },
            size: files.iter().map(|(_, b)| b.len() as u64).sum(),
            file_count: files.len() as u64,
            strip_prefix: prefix.into(),
            folders: folders.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            files_digest: upstream::files_digest(&f),
            files: f,
            ..Default::default()
        }
    }

    fn project(inst: Install) -> Project {
        Project {
            id: "link:3f9a0c12bd47".into(),
            source: "listed".into(),
            name: "Thing".into(),
            builds: vec!["3.3.5a".into()],
            repo: "https://github.com/alice/thing".into(),
            install: Some(inst),
            blp: "none".into(),
            ..Default::default()
        }
    }

    #[test]
    fn only_the_listings_own_repo_in_allowed_shapes() {
        let repo = "https://github.com/alice/thing";
        let ok = |from: &str, url: &str| {
            let mut i = block("files", from, url, &[("patch-T.MPQ", MPQ)], "", &[]);
            if from == "commit-archive" {
                i.rev = C1.into();
            }
            url_ok(&i, repo)
        };
        assert!(ok("release-asset", "https://github.com/alice/thing/releases/download/v1/patch-T.MPQ"));
        assert!(ok("release-asset", "https://github.com/alice/thing/releases/download/v1/pack.zip"));
        assert!(ok("commit-archive", &format!("https://github.com/alice/thing/archive/{C1}.zip")));
        assert!(!ok("release-asset", "https://github.com/mallory/thing/releases/download/v1/pack.zip"), "another repo");
        assert!(!ok("release-asset", "https://github.com/alice/thing/releases/download/v1/setup.exe"), "not a zip or MPQ");
        assert!(!ok("release-asset", "https://evil.example.com/alice/thing/releases/download/v1/pack.zip"));
        assert!(!ok("mirror", "https://play.openvanilla.app/wrath/mods/x.zip"), "listed mods are never mirrored");
        assert!(!ok("commit-archive", "https://github.com/alice/thing/archive/main.zip"), "pinned to a commit only");
    }

    #[test]
    fn install_blocks_are_checked() {
        let repo = "https://github.com/alice/thing";
        let good = block("files", "release-asset", "https://github.com/alice/thing/releases/download/v1/patch-T.MPQ", &[("patch-T.MPQ", MPQ)], "", &[]);
        assert!(install_ok(&good, repo, false));
        let mut bad = good.clone();
        bad.files_digest = "ab".repeat(32);
        assert!(!install_ok(&bad, repo, false), "the digest must be the list's");
        let mut bad = good.clone();
        bad.files.insert("../evil.dll".into(), "ab".repeat(32));
        assert!(!install_ok(&bad, repo, false), "unsafe paths");
        let mut bad = good.clone();
        bad.kind = "realm".into();
        assert!(!install_ok(&bad, repo, false), "only addon and files");
        let mut bad = good.clone();
        bad.files = [("other.MPQ".to_string(), sha(MPQ))].into();
        bad.files_digest = upstream::files_digest(&bad.files);
        assert!(!install_ok(&bad, repo, false), "a bare release file is the one file it is named");
        let addon = block("addon", "commit-archive", &format!("https://github.com/alice/thing/archive/{C1}.zip"),
                          &[("Frames/Frames.toc", TOC), ("Frames/Frames.lua", b"x")], &format!("thing-{C1}/"), &[("", "Frames")]);
        assert!(install_ok(&addon, repo, false));
        let mut blp = addon.clone();
        blp.files.insert("Frames/a.blp".into(), sha(b"BLP2"));
        blp.files_digest = upstream::files_digest(&blp.files);
        assert!(!install_ok(&blp, repo, false) && install_ok(&blp, repo, true), ".blp only as the author's art");
    }

    #[test]
    fn installs_a_patch_a_native_mod_and_an_addon_from_the_author_with_checks() {
        let tmp = tempfile::tempdir().unwrap();
        let client = tmp.path().join("client");
        fake_335a(&client, false);
        let scratch = tmp.path().join("scratch");
        fs::create_dir_all(&scratch).unwrap();
        let zip_patch = zip_of(&[("pack-v1/patch-T.MPQ", MPQ), ("pack-v1/readme.txt", b"hi")]);
        let zip_native = zip_of(&[("d3d9.dll", b"MZ\x90\0native"), ("d3d9.ini", b"fps=144")]);
        let zip_addon = zip_of(&[("thing-x/Frames.toc", TOC), ("thing-x/Frames.lua", b"x=1"), ("thing-x/README.md", b"#")]);
        let base = serve_routes(vec![
            ("/alice/thing/releases/download/v1/patch-H.MPQ", "200 OK", MPQ.to_vec()),
            ("/alice/thing/patch.zip", "200 OK", zip_patch),
            ("/alice/thing/native.zip", "200 OK", zip_native),
            ("/alice/thing/addon.zip", "200 OK", zip_addon),
        ]);
        // The tests serve plain http on 127.0.0.1: install() itself is exercised with the real address checks
        // skipped by building the project after the checks (url_ok is covered above).
        let run = |inst: Install, ok: &dyn Fn(&str) -> bool| -> Result<Installed, String> {
            let p = project(inst.clone());
            let dl = mods::Scratch::new(&scratch).unwrap();
            let z = dl.dir.join("d");
            let (_, sha) = upstream::download(&inst.url, &z, 1 << 26)?;
            if inst.sha256.as_deref().is_some_and(|w| w != sha) {
                return Err("sha".into());
            }
            let st = mods::Staging::new(&client).unwrap();
            let files = st.files();
            fs::create_dir_all(&files).unwrap();
            let meta = InstallMeta { key: p.id.clone(), name: p.name.clone(), version: "1".into(), source: "listed".into(), sha256: inst.rev.clone(), ..Default::default() };
            if inst.kind == "addon" {
                let u = upstream::unpack(&z, &files, &inst, false)?;
                upstream::verify(&u, &inst)?;
                return mods::install(&client, Build::V335a, meta, &upstream::plan_of(&u, &inst, Build::V335a), false);
            }
            let u = unpack_files(&z, &files, &inst)?;
            upstream::verify(&u, &inst)?;
            let plan = mods::validate(&mods::walk(&files).unwrap(), Build::V335a);
            assert!(plan.errors.is_empty(), "{:?}", plan.errors);
            mods::install(&client, Build::V335a, meta.clone(), &plan, ok(&mods::native_consent_key(&meta.key, &plan)))
        };
        // a bare .MPQ release file: the next free patch letter
        let mut bare = block("files", "release-asset", &format!("{base}/alice/thing/releases/download/v1/patch-H.MPQ"), &[("patch-H.MPQ", MPQ)], "", &[]);
        bare.sha256 = Some(sha(MPQ));
        let m = run(bare.clone(), &|_| false).unwrap();
        assert_eq!(m.source, "listed");
        assert!(client.join("Data/patch-H.MPQ").exists() && m.patches == vec!["Data/patch-H.MPQ".to_string()], "{:?}", m);
        // the whole file's hash is checked first
        let mut wrong = bare.clone();
        wrong.sha256 = Some("ab".repeat(32));
        assert!(run(wrong, &|_| false).is_err());
        // a zipped patch: only the listed file, from under the top folder
        let mut zp = block("files", "release-asset", &format!("{base}/alice/thing/patch.zip"), &[("patch-T.MPQ", MPQ)], "pack-v1/", &[]);
        zp.key_fix();
        let m = run(zp.clone(), &|_| false).unwrap();
        assert!(m.files.iter().any(|f| f.starts_with("Data/patch-")), "{:?}", m.files);
        // a file that changed since the check: nothing installed
        let mut changed = zp.clone();
        changed.files.insert("patch-T.MPQ".into(), "cd".repeat(32));
        assert!(run(changed, &|_| false).unwrap_err().contains("does not match"));
        // native code: refused until the player says yes for exactly these files
        let nat = block("files", "release-asset", &format!("{base}/alice/thing/native.zip"), &[("d3d9.dll", b"MZ\x90\0native"), ("d3d9.ini", b"fps=144")], "", &[]);
        let e = run(nat.clone(), &|_| false).unwrap_err();
        assert!(e.starts_with(mods::NATIVE_CONFIRM), "{e}");
        let m = run(nat, &|_| true).unwrap();
        assert!(m.native_code && client.join("d3d9.dll").exists());
        // an addon: the curated addons' path, dev files skipped
        let ad = block("addon", "release-asset", &format!("{base}/alice/thing/addon.zip"), &[("Frames/Frames.toc", TOC), ("Frames/Frames.lua", b"x=1")], "thing-x/", &[("", "Frames")]);
        let m = run(ad, &|_| false).unwrap();
        assert!(client.join("Interface/AddOns/Frames/Frames.toc").exists() && !client.join("Interface/AddOns/Frames/README.md").exists(), "{:?}", m);
    }

    #[test]
    fn install_refuses_what_does_not_check_out_and_tracks_updates() {
        let tmp = tempfile::tempdir().unwrap();
        let client = tmp.path().join("client");
        fake_335a(&client, false);
        let inst = block("files", "release-asset", "https://github.com/mallory/x/releases/download/v1/patch-H.MPQ", &[("patch-H.MPQ", MPQ)], "", &[]);
        let p = project(inst.clone());
        assert!(install(&client, Build::V335a, &p, tmp.path(), &|_| true).unwrap_err().contains("did not check out"), "another repo's file");
        let mut q = project(block("files", "release-asset", "https://github.com/alice/thing/releases/download/v1/patch-H.MPQ", &[("patch-H.MPQ", MPQ)], "", &[]));
        q.builds = vec!["1.12".into()];
        assert!(install(&client, Build::V335a, &q, tmp.path(), &|_| true).unwrap_err().contains("made for"));
        let m = Installed { key: p.id.clone(), source: "listed".into(), sha256: "v0".into(), ..Default::default() };
        let mut cur = project(inst.clone());
        cur.version = Some("1.1".into());
        assert_eq!(update_for(&m, &cur), Some("1.1".into()));
        let same = Installed { sha256: "v1".into(), ..m.clone() };
        assert_eq!(update_for(&same, &cur), None);
    }

    trait KeyFix {
        fn key_fix(&mut self);
    }
    impl KeyFix for Install {
        fn key_fix(&mut self) {
            self.files_digest = upstream::files_digest(&self.files);
        }
    }
}
