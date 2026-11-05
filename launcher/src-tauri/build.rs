fn main() {
    // Two cfgs every part of the crate can check, with or without the `app` feature (docs/ANDROID.md):
    //   desktop / mobile: the target is a computer, or a phone (Android, iOS). tauri-build sets the same two for the
    //     app; they are set here too so the core (`cargo test --no-default-features`) knows them as well.
    //   torrent_engine: the `torrent-engine` feature on a desktop target. The phone app never downloads or seeds game
    //     clients, and Cargo.toml only builds librqbit for desktop targets.
    println!("cargo:rustc-check-cfg=cfg(desktop)");
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    println!("cargo:rustc-check-cfg=cfg(torrent_engine)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mobile = os == "android" || os == "ios";
    println!("cargo:rustc-cfg={}", if mobile { "mobile" } else { "desktop" });
    if !mobile && std::env::var_os("CARGO_FEATURE_TORRENT_ENGINE").is_some() {
        println!("cargo:rustc-cfg=torrent_engine");
    }
    // The Tauri build step (icons, capabilities, the Windows resource) only runs for the app.
    #[cfg(feature = "app")]
    tauri_build::build();
}
