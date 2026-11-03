@echo off
setlocal
title Leyline - client-smoke (your game, started as PLAY starts it, screenshots)
rem The real desktop game on the official realm, started the way the launcher's PLAY starts it (Leyline.exe play-test
rem --official, no launcher window): the launcher's own sign-in asks the website for your game login, hands it to the
rem login screen for this one start, and the game should skip its login screen by itself. A screenshot of the game
rem every few seconds, then the game is closed and the sign-in taken back out (checked). PASS / FAIL per step.
rem Logs and screenshots: logs\client-smoke\ (summary.txt, NN-<s>s.png, play-test.json; never a password).
rem   Client smoke test.bat           character select
rem   Client smoke test.bat world     on into the world with your last character
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
if /i "%~1"=="world" (%PY% -I "tools\client-smoke\client_smoke.py" --world) else (%PY% -I "tools\client-smoke\client_smoke.py")
exit /b %ERRORLEVEL%
