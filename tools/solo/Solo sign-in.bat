@echo off
setlocal
title Leyline - solo-signin
rem The game signs in by itself: realmlist.wtf to 127.0.0.1, the account name prefilled in WTF\Config.wtf, and
rem our sign-in add-on in a patch archive of ours (backups first; solo-signin-off puts everything back).
rem   Solo sign-in.bat [game folder]   (else tools\solo\folder.txt, else the launcher's first first-era folder)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\solo\solo_proof.py" signin %*
exit /b %ERRORLEVEL%
