@echo off
setlocal
title Leyline - solo-start
rem Starts the local first-era world on 127.0.0.1 (database, login server, world server) and waits for
rem "World initialized". The servers keep running after this window closes; solo-stop stops them.
rem   Solo start.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" start %*
exit /b %ERRORLEVEL%
