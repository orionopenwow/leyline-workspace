@echo off
setlocal
title Leyline - reel-record (starts your game by itself)
rem The reel kit's takes (tools/reels/record.py, scripts/takes.json), recorded on this PC with ffmpeg: your game started
rem by the launcher's own tests (PLAY on Wrath Unleashed into the world; Your Den: open it, carry a piece, free flight;
rem the Den is saved first and put back), the live website in its own Edge window, and the launcher's pages when node
rem and Playwright are here. Leave the mouse and keyboard alone while it runs (about 10 minutes).
rem Takes: tools\reels\takes\ (not in git). Then reel-make cuts them. Nothing is posted anywhere.
rem   Reel record.bat                  every automatic take
rem   Reel record.bat game-den-carry   one take (names: py -3 tools\reels\record.py --list)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
rem ffmpeg records and cuts the videos: winget puts it in (free, from the ffmpeg builds winget lists) when it is missing
where ffmpeg >nul 2>&1 || (
  if not exist "%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*" (
    where winget >nul 2>&1 && (echo Getting ffmpeg with winget... & winget install -e --id Gyan.FFmpeg --accept-source-agreements --accept-package-agreements --disable-interactivity >nul)
  )
)
rem the newest launcher first (the same download as "Open Leyline Launcher.bat"): the takes show today's launcher
set "LDIR=%CD%\.launcher"
if not exist "%LDIR%" mkdir "%LDIR%"
where curl >nul 2>&1 && (
  if exist "%LDIR%\Leyline.exe" (curl -fsSL --max-time 120 -z "%LDIR%\Leyline.exe" -o "%LDIR%\Leyline.exe.new" "https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe" >nul 2>&1) else (curl -fsSL --max-time 300 -o "%LDIR%\Leyline.exe.new" "https://github.com/orionopenwow/Leyline/releases/download/launcher-latest/Leyline.exe")
)
if exist "%LDIR%\Leyline.exe.new" (
  for %%F in ("%LDIR%\Leyline.exe.new") do if %%~zF GTR 1000000 (
    taskkill /im Leyline.exe /f >nul 2>&1
    move /y "%LDIR%\Leyline.exe.new" "%LDIR%\Leyline.exe" >nul && echo The newest Leyline launcher is in place.
  )
  if exist "%LDIR%\Leyline.exe.new" del "%LDIR%\Leyline.exe.new" >nul 2>&1
)
%PY% -I "tools\reels\record.py" %*
exit /b %ERRORLEVEL%
