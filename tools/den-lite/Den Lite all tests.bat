@echo off
setlocal
title Leyline - Den Lite: every test
rem Every Den Lite test: the unit tests (Jaina, the builder tools, tournaments, teleports, friends hosting, placement)
rem and the integration tests (a test client logs in and plays). Owner 5 Nov: "run that for me" before the grand
rem release ships. Same Rust tools as den-lite.
cd /d "%~dp0..\..\labs\den-lite" || (echo labs\den-lite is missing. & exit /b 1)
set "TOOLS=%LOCALAPPDATA%\Leyline\client-build\tools"
set "RUSTUP_HOME=%TOOLS%\rustup"
set "CARGO_HOME=%TOOLS%\cargo"
set "PATH=%CARGO_HOME%\bin;%PATH%"
where cargo >nul 2>&1 || (echo No Rust tools found in %TOOLS%. Run the client-build step once first, or install Rust. & exit /b 1)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Leyline\den-lite-build"
echo Den Lite: every test
cargo test --release --no-fail-fast 2>&1
if errorlevel 1 (echo. & echo DEN LITE TESTS FAILED & exit /b 1)
echo.
echo DEN LITE ALL TESTS OK
exit /b 0
