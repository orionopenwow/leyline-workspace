@echo off
setlocal
title Leyline - Den replay (the mouse, frame by frame, against the model)
rem The replay harness (docs/DEN-NEXT.md, "The mouse, proven"): the owner's .moves flows and 300 random scripts played
rem against Den Lite's editor (gizmo.rs) and compared with the expected model (movemodel.rs) after every line; the
rem tale script, Den codes and Sky Island's own tests; then the same over the wire (the test client). No game, no
rem window. Same Rust tools as den-lite.
cd /d "%~dp0..\..\labs\den-lite" || (echo labs\den-lite is missing. & exit /b 1)
set "TOOLS=%LOCALAPPDATA%\Leyline\client-build\tools"
set "RUSTUP_HOME=%TOOLS%\rustup"
set "CARGO_HOME=%TOOLS%\cargo"
set "PATH=%CARGO_HOME%\bin;%PATH%"
where cargo >nul 2>&1 || (echo No Rust tools found in %TOOLS%. Run the client-build step once first, or install Rust. & exit /b 1)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Leyline\den-lite-build"
echo Den replay: the model, the scripts, the fuzz
cargo test --release --lib -- replay:: movemodel:: tale:: taleplay:: denpack:: packs:: skyisland:: 2>&1
if errorlevel 1 (echo. & echo DEN REPLAY FAILED & exit /b 1)
echo Den replay: over the wire (Sky Island, Jaina's tale, the mouse)
cargo test --release --test client -- sky_island jaina_drafts the_mouse_over_the_wire the_moves_scripts 2>&1
if errorlevel 1 (echo. & echo DEN REPLAY OVER THE WIRE FAILED & exit /b 1)
echo.
echo DEN REPLAY OK
exit /b 0
