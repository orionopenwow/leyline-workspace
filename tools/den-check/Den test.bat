@echo off
setlocal
title Leyline - den-test (starts your game by itself)
rem The real My Den, unattended: the newest Den Lite, then the launcher's own My Den code (Leyline.exe den-play --test)
rem starts Den Lite and your game, signs in, waits for the white island, then the Den's smoke test: place a piece and
rem undo it, save and load a test Den (removed after), the white room and its size, Forge's files for your Den (Forge
rem isn't opened), the saved Den checked as the site checks a shared one, an invite link and a test friend who signs
rem in with it, the autosave. Then a screenshot, the game and Den Lite closed, the one-time sign-in out. Your Den is
rem left as it was. PASS / FAIL per step with the fix. Logs: tools\den-check\logs\den-test-latest.txt and .png
rem   Den test.bat            all of it (Den test - invite.bat does the same: the invite is always in)
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
if /i "%~1"=="invite" (%PY% -I "tools\den-check\den_check.py" test --invite) else (%PY% -I "tools\den-check\den_check.py" test)
exit /b %ERRORLEVEL%
