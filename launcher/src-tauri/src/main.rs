// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "app")]
fn main() {
    mcp_first();
    // `Leyline den-play [--test]`: My Den with no launcher window (the owner's den-test step; denclick.rs)
    let args: Vec<String> = std::env::args().collect();
    if let Some(cli) = leyline_lib::denclick::parse_cli(&args) {
        std::process::exit(leyline_lib::den_play_cli(cli));
    }
    leyline_lib::run();
}

/// `Leyline mcp`: the local MCP server an agent's app starts (agents.rs), on stdin/stdout, with no window.
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
