@echo off
setlocal
title Leyline - boss-rush (read-only)
rem Read-only: Boss Rush on this PC (docs/BOSS-RUSH.md). The exact engine's own install (engine-bossrush: set up or
rem not, its raid maps), the shared engine files (the bossrush profile, the lighter map read), the ports, the light
rem version (Den Lite), the board. Nothing is changed or started; no password is shown.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\boss-rush\boss_rush.py" check
exit /b %ERRORLEVEL%
