@echo off
setlocal
title Leyline - Boss Rush tests
rem Boss Rush's tests on this PC (docs/BOSS-RUSH.md): the light version's unit tests and the test client (logs in,
rem lands on the platform, pulls, wipes off the edge, tries again, changes phase, clears) in labs\den-lite; the
rem module's catalog (g++ when it is here); the profile's and this folder's Python tests. Same Rust tools as den-lite.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "hosting\tests\test_bossrush_profile.py" || (echo. & echo BOSS RUSH TESTS FAILED: the profile & exit /b 1)
%PY% -I "tools\boss-rush\test_boss_rush.py" || (echo. & echo BOSS RUSH TESTS FAILED: the check & exit /b 1)
where g++ >nul 2>&1 && (
  g++ -std=c++17 -Wall -Wextra -Werror -I realm\custom\modules\mod-leyline-bossfights\src realm\custom\modules\mod-leyline-bossfights\tests\test_bossfights_logic.cpp -o "%TEMP%\bf_logic.exe" && "%TEMP%\bf_logic.exe" || (echo. & echo BOSS RUSH TESTS FAILED: the module's catalog & exit /b 1)
)
set "TOOLS=%LOCALAPPDATA%\Leyline\client-build\tools"
set "RUSTUP_HOME=%TOOLS%\rustup"
set "CARGO_HOME=%TOOLS%\cargo"
set "PATH=%CARGO_HOME%\bin;%PATH%"
where cargo >nul 2>&1 || (echo No Rust tools found in %TOOLS%. Run the client-build step once first, or install Rust. & exit /b 1)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Leyline\den-lite-build"
cd /d labs\den-lite || (echo labs\den-lite is missing. & exit /b 1)
cargo test --release --lib bossrush 2>&1 || (echo. & echo BOSS RUSH TESTS FAILED: the light version & exit /b 1)
cargo test --release --test client boss_rush 2>&1 || (echo. & echo BOSS RUSH TESTS FAILED: the test client & exit /b 1)
echo.
echo BOSS RUSH TESTS OK
exit /b 0
