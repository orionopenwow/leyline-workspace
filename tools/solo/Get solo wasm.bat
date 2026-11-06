@echo off
setlocal
title Leyline - solo wasm
rem The solo-wasm step (docs\PHONE-SOLO.md): Den Lite's page build from the den-lite-latest release into
rem stream\www-files\solo\, checked against its .sha256. Then press page to put it on the game page.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I tools\solo\solo_wasm.py || exit /b 1
exit /b 0
