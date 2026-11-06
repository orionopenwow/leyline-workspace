//! `den-lite-web`: Den Lite for a web page's worker (src/web.rs; docs/PHONE-SOLO.md). Only a wasm32-wasip1 build
//! makes sense: `cargo build --release --target wasm32-wasip1 --features web --bin den-lite-web`.

#[cfg(target_arch = "wasm32")]
fn main() {
    den_lite::web::main()
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("den-lite-web runs inside a web page: build it with --target wasm32-wasip1 --features web");
    std::process::exit(2)
}
