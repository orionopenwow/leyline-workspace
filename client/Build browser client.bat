@echo off
setlocal
title Leyline - build the browser client
rem Builds the browser client on this PC and puts it in play\ (dist-test parts, dist-test.parts.txt,
rem wenilla_bg.wasm.br); the "client" step ships it. Leyline Control step: client-build.
rem The work is client\build-browser-client.sh, run with Git Bash (Git for Windows). Without Git for
rem Windows, a portable Git is downloaded once into the build folder (no install, no admin).
rem Everything else it needs is downloaded once into %LOCALAPPDATA%\Leyline\client-build.
rem It asks nothing and never touches game data. First run: about an hour; later runs: less.
rem
rem   "Build browser client.bat"            build
rem   "Build browser client.bat" /dryrun    checks only: what it would download, build and write
cd /d "%~dp0"
set "MODE="
if /i "%~1"=="/dryrun" set "MODE=--dry-run"
if /i "%~1"=="--dry-run" set "MODE=--dry-run"
if /i "%~1"=="/source" set "MODE=--source-only"
if not "%~1"=="" if not defined MODE (echo Unknown option %1. Use /dryrun or nothing. & goto fail)

set "WORK=%LOCALAPPDATA%\Leyline\client-build"
if defined LEYLINE_BUILD_ROOT set "WORK=%LEYLINE_BUILD_ROOT%"
set "PORTABLE=%WORK%\tools\PortableGit"
set "GITVER=2.47.1"

echo Looking for Git Bash...
set "BASH="
if exist "%ProgramFiles%\Git\bin\bash.exe" set "BASH=%ProgramFiles%\Git\bin\bash.exe"
if not defined BASH if exist "%ProgramW6432%\Git\bin\bash.exe" set "BASH=%ProgramW6432%\Git\bin\bash.exe"
if not defined BASH if exist "%LOCALAPPDATA%\Programs\Git\bin\bash.exe" set "BASH=%LOCALAPPDATA%\Programs\Git\bin\bash.exe"
if not defined BASH if exist "%PORTABLE%\bin\bash.exe" set "BASH=%PORTABLE%\bin\bash.exe"
if defined BASH goto have_bash
if "%MODE%"=="--dry-run" (
  echo No Git for Windows here: a real run downloads portable Git %GITVER% into %PORTABLE%.
  echo The dry run stops here.
  goto done
)
where curl.exe >nul 2>nul || (echo curl.exe is missing: this needs Windows 10 or newer. & goto fail)
echo Downloading portable Git %GITVER% (once, about 60 MB)...
if not exist "%WORK%\tools" mkdir "%WORK%\tools" || goto fail
curl.exe -fsSL --retry 3 -o "%WORK%\tools\PortableGit.7z.exe" "https://github.com/git-for-windows/git/releases/download/v%GITVER%.windows.1/PortableGit-%GITVER%-64-bit.7z.exe" || (echo Could not download portable Git. Check the internet connection and run this again. & goto fail)
"%WORK%\tools\PortableGit.7z.exe" -o"%PORTABLE%" -y >nul || (echo Could not unpack portable Git. & goto fail)
del "%WORK%\tools\PortableGit.7z.exe" >nul 2>nul
if not exist "%PORTABLE%\bin\bash.exe" (echo Portable Git has no bash.exe. & goto fail)
set "BASH=%PORTABLE%\bin\bash.exe"

:have_bash
echo Using %BASH%
set "SCRIPT=%~dp0build-browser-client.sh"
set "SCRIPT=%SCRIPT:\=/%"
"%BASH%" "%SCRIPT%" %MODE% || goto fail

:done
echo.
pause
exit /b 0

:fail
echo.
echo The build stopped; the client in play\ is unchanged. Send Claude this window's text.
pause
exit /b 1
