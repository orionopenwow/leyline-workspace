@echo off
setlocal
title Leyline - den-features (starts your game by itself)
rem The Den editor's feature test: the same start as "Den test.bat" (the newest launcher and Den Lite, your game
rem signs in by itself, the Den's smoke test), then every editor feature through the Leyline UI's own buttons: the
rem gallery (do the tiles show models?), five props placed, select, move, turn, size, copy, paste, duplicate, floor,
rem delete, undo, redo, save and open by name, every panel, play mode; a screenshot after each and PASS / FAIL with
rem the fix. Your Den is saved first and put back after. Needs the Leyline addons and Den Lite from the latest Ship.
rem Logs: tools\den-check\logs\den-features-latest.txt, the pictures in tools\den-check\logs\den-features\
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
rem the newest launcher first (the same download as "Open Leyline Launcher.bat"), so the test runs today's My Den
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
%PY% -I "tools\den-check\den_check.py" features
exit /b %ERRORLEVEL%
