@echo off
setlocal
title Leyline - solo-check (read-only)
rem Read-only: is the first-era game folder right for Leyline Solo? The game's build, its archives, the sign-in
rem slot, the login screen's file list, free disk, the ports. Changes nothing.
rem   Solo check.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" check %*
exit /b %ERRORLEVEL%
