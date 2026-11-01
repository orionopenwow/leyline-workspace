@echo off
setlocal
title Leyline - build Den Lite on this PC
rem Builds Den Lite (labs\den-lite: the tiny Den server, no Docker) with the Rust tools the browser-client build
rem already installed on this PC (%LOCALAPPDATA%\Leyline\client-build\tools), and puts den-lite.exe where the
rem launcher's Your Den looks for it: %APPDATA%\Leyline\den-lite\bin\den-lite.exe. Changes nothing else.
cd /d "%~dp0"
set "TOOLS=%LOCALAPPDATA%\Leyline\client-build\tools"
set "RUSTUP_HOME=%TOOLS%\rustup"
set "CARGO_HOME=%TOOLS%\cargo"
set "PATH=%CARGO_HOME%\bin;%PATH%"
where cargo >nul 2>&1 || (echo No Rust tools found in %TOOLS%. Run the client-build step once first, or install Rust. & exit /b 1)
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\Leyline\den-lite-build"
echo Building Den Lite (a few minutes the first time)...
cargo build --release --locked 2>&1 || cargo build --release 2>&1 || (echo The build failed: see above. & exit /b 1)
if not exist "%APPDATA%\Leyline\den-lite\bin" mkdir "%APPDATA%\Leyline\den-lite\bin"
copy /y "%CARGO_TARGET_DIR%\release\den-lite.exe" "%APPDATA%\Leyline\den-lite\bin\den-lite.exe" >nul || (echo Could not copy den-lite.exe. & exit /b 1)
echo.
echo Den Lite is at %APPDATA%\Leyline\den-lite\bin\den-lite.exe
echo In the launcher: Your Den (after the launcher update with Your Den). Login: account DEN, password den.
echo DEN LITE BUILT
exit /b 0
