@echo off
setlocal
title Leyline - solo-signin-off
rem Puts the game folder back exactly as it was before solo-signin (every byte) and removes our archive.
rem   Solo sign-in off.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" signin --off %*
exit /b %ERRORLEVEL%
