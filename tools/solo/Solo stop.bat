@echo off
setlocal
title Leyline - solo-stop
rem Stops the local world cleanly (the world server saves first) and takes the password out of the sign-in
rem archive (solo-signin writes it again).
rem   Solo stop.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" stop %*
exit /b %ERRORLEVEL%
