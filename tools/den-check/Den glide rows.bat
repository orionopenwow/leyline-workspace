@echo off
setlocal
title Leyline - the gliding copy's rows into this PC's game (den-glide)
rem Den Lite's glide-rows (labs\den-lite\src\gliderows.rs): for each prop model, the creature rows the launcher's Den patch
rem will have (host\creaturemodels.rs), made from THIS PC's own game tables into Data\patch-S.MPQ (ours, backup first), so
rem the copy in hand glides in Your Den now. Run den-lite first (the Den Lite built here has the command).
rem "--undo" (or the den-glide-undo step) takes them away. Close the game first.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
set "MODE=glide"
if /i "%~1"=="--undo" set "MODE=glide-undo"
%PY% -I "tools\den-check\den_check.py" %MODE%
exit /b %ERRORLEVEL%
