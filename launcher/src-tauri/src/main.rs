// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(feature = "app", desktop))]
fn main() {
    solo_dir();
    mcp_first();
    // `Leyline den-play [--test]`: My Den with no launcher window (the owner's den-test step; denclick.rs)
    let args: Vec<String> = std::env::args().collect();
    if let Some(cli) = leyline_lib::denclick::parse_cli(&args) {
        std::process::exit(leyline_lib::den_play_cli(cli));
    }
    // `Leyline play-test --official [--hold <s>]`: PLAY on the official realm with no launcher window (the owner's
    // client-smoke step; playtest.rs)
    match leyline_lib::playtest::parse_cli(&args) {
        Some(Ok(cli)) => {
            // --fresh: an empty data folder for this run, before anything reads the settings (the first-run tool
            // sets LEYLINE_APP_DIR itself, so it knows where play-test.json lands; else one under the temp folder)
            let set = std::env::var_os(leyline_lib::settings::APP_DIR_ENV).is_some_and(|v| std::path::Path::new(&v).is_absolute());
            if cli.fresh && !set {
                let d = leyline_lib::playtest::fresh_dir(&std::env::temp_dir(), leyline_lib::denclick::now_ms());
                let _ = std::fs::create_dir_all(&d);
                std::env::set_var(leyline_lib::settings::APP_DIR_ENV, &d);
            }
            std::process::exit(leyline_lib::play_test_cli(cli))
        }
        Some(Err(e)) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
        None => {}
    }
    leyline_lib::run();
}

/// Leyline Solo keeps its own data folder (%APPDATA%\\Leyline Solo and the like), so it never shares settings, game
/// folders or worlds with the Leyline Launcher on the same PC. An absolute LEYLINE_APP_DIR still wins (tests).
#[cfg(all(feature = "app", desktop))]
fn solo_dir() {
    #[cfg(feature = "solo")]
    {
        let key = leyline_lib::settings::APP_DIR_ENV;
        let set = std::env::var_os(key).is_some_and(|v| std::path::Path::new(&v).is_absolute());
        if !set {
            if let Some(d) = dirs::data_dir() {
                std::env::set_var(key, d.join("Leyline Solo"));
            }
        }
    }
}

/// The phone app starts in the library (`mobile_entry_point`, mobile.rs); this is only here so the binary builds
/// for a phone target too.
#[cfg(all(feature = "app", mobile))]
fn main() {
    leyline_lib::run();
}

/// `Leyline mcp`: the local MCP server an agent's app starts (agents.rs), on stdin/stdout, with no window.
#[cfg(any(desktop, not(feature = "app")))]
fn mcp_first() {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        std::process::exit(leyline_lib::agents::serve_stdio());
    }
}

#[cfg(not(feature = "app"))]
fn main() {
    mcp_first();
    eprintln!("Leyline was built without the `app` feature: only the core library is available.");
    std::process::exit(2);
}
