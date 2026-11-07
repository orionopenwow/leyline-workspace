@echo off
setlocal
title Leyline - solo-first-run (Leyline Solo on a fresh profile, through to character select)
rem Leyline Solo's fresh-machine test (docs/FIRST-RUN-TEST.md part 5): a new empty profile for the app and the world
rem (never your own: it refuses your own Solo and launcher folders, a used folder, the Solo ports in use, and your
rem own sign-in archive in the game folder), your first-era game folder checked, setup (up to an hour), the world
rem started, the game logging in by itself to character select. Then it WATCHES while you Quick start and type
rem .bot group (it never presses anything), and puts everything back: the game closed, the world stopped, sign-in
rem off, your game folder byte for byte, the test folder removed. PASS / FAIL with the fix.
rem Logs: tools\first-run\logs\solo-first-run-latest.txt and solo-first-run.json
rem   Solo first run test.bat                          all of it (the game folder from tools\solo\folder.txt)
rem   Solo first run test.bat --game "D:\Games\Old"    that folder
rem   Solo first run test.bat --zip <engine zip>       a local engine zip, not the download
rem   Solo first run test.bat --installer <setup.exe>  also the silent install into the test folder
rem   Solo first run test.bat --hold 900 --keep        watch longer; keep the test folder
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\solo_first_run.py" run %*
exit /b %ERRORLEVEL%
