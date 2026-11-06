@echo off
setlocal
title Leyline - bots-smoke (a throwaway realm, then removed)
rem The bot smoke test (docs\BOTS-UPDATES.md) on this PC: a throwaway realm from the canary images (or the tag given:
rem   Bots smoke test.bat stable), your own extracted game data volume (ov-client-data, read only), a headless
rem player with a test account made by the realm's own console: bots join, go into a dungeon, follow, fight one pull,
rem leave. PASS / FAIL per check. Everything it made is removed at the end. Changes nothing of your realms.
cd /d "%~dp0.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
set "TAG=%~1"
if not defined TAG set "TAG=canary"
if not exist "logs" mkdir "logs"
%PY% -I "realm\bots\bots_smoke.py" smoke --tag "%TAG%" --out "logs\bots-smoke-latest.json"
exit /b %ERRORLEVEL%
