@echo off
setlocal
title Leyline - Project Reforged for your Den: download
rem Downloads Project Reforged's 3.3.5a patches from their own links (https://projectreforged.github.io/) into
rem Desktop\Leyline\looks\reforged-335. The game never reads that folder: only "Play my Den with Project Reforged" puts
rem them in, for your Den only. Default set: characters, buildings, creatures, doodads, environment, gear, interface,
rem spell effects. Add more with: Den looks - get Project Reforged.bat MNU (maps, darker nights, upright orcs).
rem Safe to run again: it goes on where it stopped and skips what is already here.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & pause & exit /b 1)
%PY% -I "tools\den-looks\den_looks.py" get --with "%~1"
echo.
pause
exit /b %ERRORLEVEL%
