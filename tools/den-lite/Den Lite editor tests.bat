@echo off
setlocal
title Leyline - Den Lite editor tests (the mouse, the arrows, the ground)
rem Only Den Lite's editor tests: gizmo.rs (click to carry, the arrows, hover, drag, the debug line) and pick.rs (the
rem cursor's ray, the ground). Not the whole Den test (owner 4 Nov: "one test at a time"). Same Rust tools as den-lite.
cd /d "%~dp0..\..\labs\den-lite" || (echo labs\den-lite is missing. & exit /b 1)
set "TOOLS=%LOCALAPPDATA%\Leyline\client-build\tools"
set "RUSTUP_HOME=%TOOLS%\rustup"
set "CARGO_HOME=%TOOLS%\cargo"
set "PATH=%CARGO_HOME%\bin;%PATH%"
where cargo >nul 2>&1 || (echo No Rust tools found in %TOOLS%. Run the client-build step once first, or install Rust. & exit /b 1)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Leyline\den-lite-build"
echo Den Lite editor tests: gizmo, pick and the replay against the model
cargo test --release --lib -- gizmo:: pick:: replay:: movemodel:: 2>&1
if errorlevel 1 (echo. & echo DEN LITE EDITOR TESTS FAILED & exit /b 1)
echo.
echo DEN LITE EDITOR TESTS OK
exit /b 0
