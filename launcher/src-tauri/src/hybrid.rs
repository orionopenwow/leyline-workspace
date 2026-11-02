//! The hybrid client (docs/HYBRID-CLIENT.md): the player's own desktop game starts like the browser game. Behind
//! Settings' `hybrid_login` switch (off by default), for 3.3.5a game folders that already have Forever UI's login
//! screens (its installer or "play\Install Forever UI.bat" put them in `Interface/GlueXML`).
//!
//! What it does: one patch archive, `Data/patch-Z.MPQ` (the one the game reads last; Forever UI's own installer
//! uses the same name), written by `crate::mpq` from
//! * Forever UI's `GlueXML.toc` and its `ForeverUI*` glue files, as they are in the player's folder (never shipped
//!   by us), with one line added: `LeylineHybrid.xml`, after Forever UI's own glue;
//! * our own files (`launcher/src-tauri/glue/`): `LeylineHybrid.xml`, `LeylineHybrid.lua` and the Leyline mark;
//! * `LeylineSession.lua`: for one start of the game, the one-time login the realm made for it (Hero Key sign-in,
//!   `crate::herokey`), every byte as a `\ddd` escape; otherwise one line that sets nothing.
//!
//! The session is taken out again (`scrub`) as soon as the game lets go of the archive (when it closes; the
//! launcher retries until then), and on the next write. The password only ever works once and for minutes (the
//! realm scrambles it after `login_ttl`), but it still never stays on disk longer than that one start.
//!
//! An archive in that slot that Leyline did not write (no `LeylineHybrid.lua` inside, or bigger than ours can be)
//! is never touched: the switch then says why and the old way (type the one-time password) is used.

use crate::mpq;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const SLOT: &str = "patch-Z.MPQ";
pub const OUR_XML: &str = "LeylineHybrid.xml";
const OUR_LUA_NAME: &str = "Interface\\GlueXML\\LeylineHybrid.lua";
const SESSION_NAME: &str = "Interface\\GlueXML\\LeylineSession.lua";
/// Ours is a few hundred KB; anything much bigger in the slot is someone else's (Forever UI's own is ~330 MB).
const OURS_MAX: u64 = 32 * 1024 * 1024;
const NO_SESSION: &str = "-- Leyline: no sign-in for this start (the launcher writes one only for the start it makes).\r\nLEYLINE_SESSION = nil\r\n";

const OUR_FILES: &[(&str, &[u8])] = &[
    ("Interface\\GlueXML\\LeylineHybrid.xml", include_bytes!("../glue/LeylineHybrid.xml")),
    (OUR_LUA_NAME, include_bytes!("../glue/LeylineHybrid.lua")),
    ("Interface\\Glues\\Leyline\\LeylineMark.tga", include_bytes!("../glue/LeylineMark.tga")),
];

/// One start's sign-in.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub user: String,
    pub pass: String,
    /// The realm's name on its realm list, picked when the list shows (None: the game's own choice).
    pub realm: Option<String>,
    /// Enter the world with the last character once character select is up.
    pub enter: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Status {
    /// Forever UI's login screens are in this folder.
    pub foreverui: bool,
    /// The slot is free or ours.
    pub slot_ok: bool,
    /// Our archive is there.
    pub installed: bool,
    pub why: String,
}

fn lua_bytes(s: &str) -> String {
    let mut o = String::with_capacity(s.len() * 4 + 2);
    o.push('"');
    for b in s.bytes() {
        o.push_str(&format!("\\{b:03}"));
    }
    o.push('"');
    o
}

/// `LeylineSession.lua`: the session as escaped bytes (nothing of it readable as plain text), or the empty one.
/// Only an account name and password of the shapes the realm makes are written.
pub fn session_lua(s: Option<&Session>) -> Result<String, String> {
    let Some(s) = s else { return Ok(NO_SESSION.to_string()) };
    let printable = |v: &str, max: usize| (1..=max).contains(&v.len()) && v.bytes().all(|b| (0x21..=0x7e).contains(&b));
    if !printable(&s.user, 32) || !printable(&s.pass, 16) {
        return Err("That login can't be handed to the game.".into());
    }
    let realm = match &s.realm {
        Some(r) if (1..=64).contains(&r.len()) && !r.chars().any(char::is_control) => lua_bytes(r),
        Some(_) => return Err("That realm name can't be handed to the game.".into()),
        None => "nil".into(),
    };
    Ok(format!(
        "-- Leyline: the sign-in for one start of the game, made by the launcher and removed when the game closes.\r\n\
         LEYLINE_SESSION = {{ user = {}, pass = {}, realm = {realm}, enter = {} }}\r\n",
        lua_bytes(&s.user),
        lua_bytes(&s.pass),
        s.enter
    ))
}

/// Forever UI's `GlueXML.toc` with `LeylineHybrid.xml` after its last `ForeverUI*.xml` line (once).
pub fn toc_with_ours(toc: &str) -> Result<String, String> {
    let nl = if toc.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<&str> = toc.lines().filter(|l| !l.trim().eq_ignore_ascii_case(OUR_XML)).collect();
    let last = lines.iter().rposition(|l| {
        let t = l.trim().to_ascii_lowercase();
        t.starts_with("foreverui") && t.ends_with(".xml") && !t.contains(['\\', '/'])
    });
    let Some(i) = last else {
        return Err("This GlueXML.toc isn't Forever UI's (it doesn't list ForeverUIGlue.xml).".into());
    };
    lines.insert(i + 1, OUR_XML);
    Ok(lines.join(nl) + nl)
}

/// Forever UI's glue in the player's folder: `GlueXML.toc` and the `ForeverUI*.lua` / `.xml` files beside it.
pub fn foreverui_glue(client: &Path) -> Result<(String, Vec<(String, Vec<u8>)>), String> {
    let dir = crate::clients::find_rel(client, "Interface/GlueXML")
        .ok_or("Forever UI's login screens aren't in this game folder yet (install Forever UI first).")?;
    let toc_path = crate::clients::find_ci(&dir, "GlueXML.toc").ok_or("Forever UI's GlueXML.toc isn't in Interface/GlueXML.")?;
    let toc = fs::read(&toc_path).map_err(|e| format!("Couldn't read GlueXML.toc: {e}"))?;
    if toc.len() > 64 * 1024 {
        return Err("That GlueXML.toc is too big to be Forever UI's.".into());
    }
    let toc = String::from_utf8_lossy(&toc).to_string();
    let mut files = Vec::new();
    let mut rd: Vec<_> = fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten().collect();
    rd.sort_by_key(|e| e.file_name());
    for e in rd {
        let n = e.file_name().to_string_lossy().to_string();
        let low = n.to_ascii_lowercase();
        if !(low.starts_with("foreverui") && (low.ends_with(".lua") || low.ends_with(".xml"))) || !e.path().is_file() {
            continue;
        }
        let name = format!("Interface\\GlueXML\\{n}");
        if !mpq::name_ok(&name) {
            continue;
        }
        let data = fs::read(e.path()).map_err(|err| format!("Couldn't read {n}: {err}"))?;
        if data.len() > 4 * 1024 * 1024 {
            return Err(format!("{n} is too big to be a login screen file."));
        }
        files.push((name, data));
    }
    if !files.iter().any(|(n, _)| n.eq_ignore_ascii_case("Interface\\GlueXML\\ForeverUIGlue.xml")) {
        return Err("Forever UI's ForeverUIGlue.xml isn't in Interface/GlueXML.".into());
    }
    Ok((toc, files))
}

/// Everything the archive holds, for this session (or none).
pub fn bundle(client: &Path, s: Option<&Session>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let (toc, fui) = foreverui_glue(client)?;
    let mut files = vec![("Interface\\GlueXML\\GlueXML.toc".to_string(), toc_with_ours(&toc)?.into_bytes())];
    files.extend(fui);
    files.extend(OUR_FILES.iter().map(|(n, d)| (n.to_string(), d.to_vec())));
    files.push((SESSION_NAME.to_string(), session_lua(s)?.into_bytes()));
    Ok(files)
}

fn data_dir(client: &Path) -> Result<PathBuf, String> {
    crate::clients::find_ci(client, "Data").filter(|d| d.is_dir()).ok_or_else(|| "This game folder has no Data folder.".into())
}

/// `<game>/Leyline/glue-refused` (app.rs den_watch): this game closed on our login screen code once (a game
/// program that checks its login screen files), so the archive is never written for it again.
pub fn refused_marker(client: &Path) -> PathBuf {
    client.join("Leyline").join("glue-refused")
}

/// The locale folders under Data (`enUS`, `deDE`...): the game reads `Data/<locale>/patch-<locale>-<X>.MPQ` too.
pub fn locales(client: &Path) -> Vec<String> {
    let Ok(data) = data_dir(client) else { return Vec::new() };
    let mut v: Vec<String> = fs::read_dir(&data)
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default();
    v.retain(|n| n.len() == 4 && n[..2].bytes().all(|b| b.is_ascii_lowercase()) && n[2..].bytes().all(|b| b.is_ascii_uppercase()));
    v.sort();
    v
}

fn ours_at(p: &Path) -> bool {
    let len = fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX);
    len <= OURS_MAX && fs::read(p).ok().is_some_and(|b| mpq::read(&b, OUR_LUA_NAME).is_some())
}

/// The slot's path when it is free or holds an archive Leyline wrote; why not, otherwise.
pub fn slot(client: &Path) -> Result<(PathBuf, bool), String> {
    let data = data_dir(client)?;
    let Some(p) = crate::clients::find_ci(&data, SLOT) else { return Ok((data.join(SLOT), false)) };
    if ours_at(&p) {
        Ok((p, true))
    } else {
        Err(format!("Data/{SLOT} belongs to another mod (Forever UI's own installer puts one there), so Leyline leaves it alone."))
    }
}

/// Every place our archive goes: `Data/patch-Z.MPQ`, and the same archive as `Data/<locale>/patch-<locale>-Z.MPQ`
/// for each locale folder (sources disagree on whether the root or the locale patches win over each other; with
/// both, ours is the last word either way). A locale slot holding someone else's archive is left out.
pub fn slots(client: &Path) -> Result<Vec<(PathBuf, bool)>, String> {
    let mut v = vec![slot(client)?];
    let data = data_dir(client)?;
    for loc in locales(client) {
        let Some(dir) = crate::clients::find_ci(&data, &loc) else { continue };
        let name = format!("patch-{loc}-Z.MPQ");
        match crate::clients::find_ci(&dir, &name) {
            None => v.push((dir.join(name), false)),
            Some(p) if ours_at(&p) => v.push((p, true)),
            Some(_) => {}
        }
    }
    Ok(v)
}

/// Whether this session is in our archive now (tests and the status line).
pub fn has_session(client: &Path) -> bool {
    let Ok(all) = slots(client) else { return false };
    all.iter()
        .filter(|(_, ours)| *ours)
        .any(|(p, _)| fs::read(p).ok().and_then(|b| mpq::read(&b, SESSION_NAME)).is_some_and(|s| s.as_slice() != NO_SESSION.as_bytes()))
}

/// Write the archive for this start. Fails, changing nothing, when Forever UI's glue isn't there, the slot is
/// someone else's, the game refused our login screen code before, or the game holds the archive open (it is
/// running). Returns the root slot's path.
pub fn write(client: &Path, s: Option<&Session>) -> Result<PathBuf, String> {
    if s.is_some() && refused_marker(client).exists() {
        return Err("This game doesn't take Leyline's login screen (it closed on it once).".into());
    }
    let all = slots(client)?;
    let bytes = mpq::build(&bundle(client, s)?)?;
    for (p, _) in &all {
        crate::settings::write_atomic(p, &bytes).map_err(|e| format!("Couldn't write {} (is the game still running?): {e}", p.display()))?;
    }
    Ok(all[0].0.clone())
}

/// Take the session out (the archive stays, with the empty one). Ok(true): it was taken out; Ok(false): there was
/// none. Err while the game still holds the archive.
pub fn scrub(client: &Path) -> Result<bool, String> {
    if !has_session(client) {
        return Ok(false);
    }
    write(client, None).map(|_| true)
}

/// Take our archive away (the switch turned off), from every slot. Someone else's is never touched.
pub fn remove(client: &Path) -> Result<bool, String> {
    let mut any = false;
    let all = slots(client).or_else(|_| {
        // the root slot is someone else's: our locale ones still go
        let data = data_dir(client)?;
        Ok::<_, String>(locales(client).into_iter().filter_map(|l| {
            let p = crate::clients::find_ci(&data, &l)?.join(format!("patch-{l}-Z.MPQ"));
            let ours = ours_at(&p);
            Some((p, ours))
        }).collect())
    }).unwrap_or_default();
    for (p, ours) in all {
        if ours {
            fs::remove_file(&p).map_err(|e| format!("Couldn't remove {}: {e}", p.display()))?;
            any = true;
        }
    }
    Ok(any)
}

pub fn status(client: &Path) -> Status {
    let fui = foreverui_glue(client);
    let sl = slot(client);
    let why = match (&fui, &sl) {
        (Err(e), _) => e.clone(),
        (_, Err(e)) => e.clone(),
        _ => String::new(),
    };
    Status { foreverui: fui.is_ok(), slot_ok: sl.is_ok(), installed: matches!(sl, Ok((_, true))), why }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUI_TOC: &str = "## Interface: 30300\r\nGlueLocalization.xml\r\nGlueParent.xml\r\nForeverUIGlue.xml\r\nGlueLocalizationPost.xml\r\n";

    fn client() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("Data/enUS")).unwrap();
        let g = d.path().join("Interface/GlueXML");
        fs::create_dir_all(&g).unwrap();
        fs::write(g.join("GlueXML.toc"), FUI_TOC).unwrap();
        fs::write(g.join("ForeverUIGlue.xml"), "<Ui><Script file=\"ForeverUIGlue.lua\"/></Ui>").unwrap();
        fs::write(g.join("ForeverUIGlue.lua"), "ForeverUIGlue = {}").unwrap();
        fs::write(g.join("AccountLogin.lua"), "not ours, not Forever UI's: never packed").unwrap();
        d
    }

    fn sess() -> Session {
        Session { user: "HKABCDEFGHIJKLMN".into(), pass: "p@ss\"]]..\\x".into(), realm: Some("Den of \"Q\"".into()), enter: true }
    }

    #[test]
    fn the_toc_gets_our_line_once_after_forever_uis() {
        let t = toc_with_ours(FUI_TOC).unwrap();
        assert_eq!(t, "## Interface: 30300\r\nGlueLocalization.xml\r\nGlueParent.xml\r\nForeverUIGlue.xml\r\nLeylineHybrid.xml\r\nGlueLocalizationPost.xml\r\n");
        assert_eq!(toc_with_ours(&t).unwrap(), t, "idempotent");
        let two = "A.xml\nForeverUIGlue.xml\nForeverUIGlueWeb.xml\nB.xml\n";
        assert_eq!(toc_with_ours(two).unwrap(), "A.xml\nForeverUIGlue.xml\nForeverUIGlueWeb.xml\nLeylineHybrid.xml\nB.xml\n");
        assert!(toc_with_ours("GlueParent.xml\r\n").is_err(), "a stock toc is refused");
    }

    #[test]
    fn the_session_is_escaped_bytes_only() {
        let s = session_lua(Some(&sess())).unwrap();
        assert!(!s.contains("HKABC") && !s.contains("p@ss") && !s.contains("Den of"), "nothing readable as plain text");
        assert!(s.contains("enter = true"));
        assert!(s.contains("\\072\\075"), "H K");
        assert_eq!(session_lua(None).unwrap(), NO_SESSION);
        for bad in [Session { pass: "".into(), ..sess() }, Session { pass: "a b".into(), ..sess() }, Session { user: "x\n".into(), ..sess() },
            Session { pass: "x".repeat(17), ..sess() }, Session { realm: Some("a\nb".into()), ..sess() }] {
            assert!(session_lua(Some(&bad)).is_err(), "{bad:?}");
        }
        // it runs in a real Lua 5.1 and gives back exactly the values (when one is installed here)
        let lua = ["lua5.1", "lua"].iter().find(|l| std::process::Command::new(l).arg("-v").output().is_ok());
        if let Some(lua) = lua {
            let code = format!("{s}\nio.write(LEYLINE_SESSION.user, '|', LEYLINE_SESSION.pass, '|', LEYLINE_SESSION.realm, '|', tostring(LEYLINE_SESSION.enter))");
            let out = std::process::Command::new(lua).arg("-e").arg(code).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), "HKABCDEFGHIJKLMN|p@ss\"]]..\\x|Den of \"Q\"|true");
        }
    }

    /// The glue's own tests (glue/test_hybrid_glue.lua, a stand-in for the login screens) in a real Lua 5.1, when
    /// one is installed here (CI without Lua skips them; `lua5.1 glue/test_hybrid_glue.lua glue/LeylineHybrid.lua`).
    #[test]
    fn the_glue_signs_in_once_and_steps_aside_on_anything_unexpected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("glue");
        let Some(lua) = ["lua5.1", "lua"].iter().find(|l| std::process::Command::new(l).arg("-v").output().is_ok()) else { return };
        let out = std::process::Command::new(lua).arg(root.join("test_hybrid_glue.lua")).arg(root.join("LeylineHybrid.lua")).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success() && text.contains("all passed"), "{text}{}", String::from_utf8_lossy(&out.stderr));
    }

    #[test]
    fn write_scrub_and_remove() {
        let d = client();
        let p = write(d.path(), Some(&sess())).unwrap();
        assert_eq!(p, d.path().join("Data").join(SLOT));
        let a = fs::read(&p).unwrap();
        let toc = String::from_utf8(mpq::read(&a, "Interface\\GlueXML\\GlueXML.toc").unwrap()).unwrap();
        assert!(toc.contains("ForeverUIGlue.xml\r\nLeylineHybrid.xml\r\n"));
        assert_eq!(mpq::read(&a, "Interface\\GlueXML\\ForeverUIGlue.lua").unwrap(), b"ForeverUIGlue = {}");
        assert!(mpq::read(&a, "Interface\\GlueXML\\AccountLogin.lua").is_none(), "only Forever UI's files and ours");
        assert_eq!(mpq::read(&a, OUR_LUA_NAME).unwrap(), include_bytes!("../glue/LeylineHybrid.lua"));
        assert!(mpq::read(&a, "Interface\\Glues\\Leyline\\LeylineMark.tga").is_some());
        assert!(has_session(d.path()));
        assert!(status(d.path()).installed);
        // after the game closes: the session goes, the rest stays
        assert!(scrub(d.path()).unwrap());
        assert!(!has_session(d.path()));
        assert_eq!(mpq::read(&fs::read(&p).unwrap(), SESSION_NAME).unwrap(), NO_SESSION.as_bytes());
        assert!(!scrub(d.path()).unwrap(), "nothing left to take out");
        assert!(remove(d.path()).unwrap());
        assert!(!p.exists());
        assert!(!remove(d.path()).unwrap());
    }

    #[test]
    fn the_same_archive_goes_in_the_locale_folder_too_and_a_refusal_is_remembered() {
        let d = client();
        assert_eq!(locales(d.path()), ["enUS"]);
        fs::create_dir_all(d.path().join("Data/Cache")).unwrap(); // not a locale
        assert_eq!(locales(d.path()), ["enUS"]);
        write(d.path(), Some(&sess())).unwrap();
        let root = fs::read(d.path().join("Data").join(SLOT)).unwrap();
        let loc = fs::read(d.path().join("Data/enUS/patch-enUS-Z.MPQ")).unwrap();
        assert_eq!(root, loc, "one archive in both places");
        assert!(scrub(d.path()).unwrap());
        assert_eq!(mpq::read(&fs::read(d.path().join("Data/enUS/patch-enUS-Z.MPQ")).unwrap(), SESSION_NAME).unwrap(), NO_SESSION.as_bytes(), "scrubbed in both");
        // someone else's locale patch is left alone, ours still goes in the root
        fs::write(d.path().join("Data/enUS/patch-enUS-Z.MPQ"), b"theirs").unwrap();
        write(d.path(), Some(&sess())).unwrap();
        assert_eq!(fs::read(d.path().join("Data/enUS/patch-enUS-Z.MPQ")).unwrap(), b"theirs");
        assert!(remove(d.path()).unwrap());
        assert_eq!(fs::read(d.path().join("Data/enUS/patch-enUS-Z.MPQ")).unwrap(), b"theirs");
        // the game closed on our login screen code once: never written with a session again
        fs::create_dir_all(d.path().join("Leyline")).unwrap();
        fs::write(refused_marker(d.path()), "x").unwrap();
        assert!(write(d.path(), Some(&sess())).unwrap_err().contains("doesn't take"));
        assert!(!d.path().join("Data").join(SLOT).exists());
    }

    /// tools/den-check/fixtures/patch-Z-sample.bin is exactly what this writer makes for the stand-in folder above
    /// (our own files only), so tools/den-check's own reader and an independent one are tested against the real
    /// output. `LEYLINE_WRITE_FIXTURES=1 cargo test` writes it again after a change to the writer or the glue.
    #[test]
    fn the_den_check_sample_archive_is_this_writers_output() {
        let d = client();
        let bytes = mpq::build(&bundle(d.path(), None).unwrap()).unwrap();
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/den-check/fixtures/patch-Z-sample.bin");
        if std::env::var("LEYLINE_WRITE_FIXTURES").is_ok() {
            fs::create_dir_all(fx.parent().unwrap()).unwrap();
            fs::write(&fx, &bytes).unwrap();
        }
        let Ok(have) = fs::read(&fx) else { return }; // not in a crate-only checkout
        assert!(have == bytes, "the sample archive is out of date: run LEYLINE_WRITE_FIXTURES=1 cargo test the_den_check_sample");
    }

    #[test]
    fn someone_elses_archive_and_a_folder_without_forever_ui_are_left_alone() {
        let d = client();
        let theirs = d.path().join("Data").join(SLOT);
        let other = mpq::build(&[("Interface\\ForeverUI\\x.blp".into(), b"art".to_vec())]).unwrap();
        fs::write(&theirs, &other).unwrap();
        assert!(write(d.path(), Some(&sess())).is_err());
        assert!(!remove(d.path()).unwrap());
        assert_eq!(fs::read(&theirs).unwrap(), other, "untouched");
        assert!(!status(d.path()).slot_ok);

        let e = tempfile::tempdir().unwrap();
        fs::create_dir_all(e.path().join("Data")).unwrap();
        assert!(write(e.path(), Some(&sess())).is_err());
        assert!(!e.path().join("Data").join(SLOT).exists());
        assert!(!status(e.path()).foreverui);
    }
}
