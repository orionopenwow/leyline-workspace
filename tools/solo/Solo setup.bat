@echo off
setlocal
title Leyline - solo-setup
rem Leyline Solo, Phase 0: the engine from solo-engine-latest (checked against its .sha256), the maps made from
rem YOUR game folder (read only), the database, the local account SOLO. 20-60 minutes the first time.
rem Add --mmaps for the monster paths (1-3 hours more).
rem   Solo setup.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" setup %*
exit /b %ERRORLEVEL%
