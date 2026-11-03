@echo off
setlocal
title Make my own Leyline
rem Make your own Leyline: a few questions (name, colours, logo, domain, Discord), then your own website and Log in
rem page, browser kit, host kit and launcher branding, ready to upload. Leyline's own files are never changed.
rem Needs Python 3 (python.org). Output: tools\commons\out\<your name>\ (UPLOAD.md there says what to do next).
rem No game files are ever packed: the kit refuses any it finds.
cd /d "%~dp0"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed. Get it from https://www.python.org/downloads/ then run this again. & pause & exit /b 1)
%PY% -I "%~dp0commons.py" make %*
if errorlevel 1 (echo. & echo Nothing was made. The lines above say why. & pause & exit /b 1)
echo.
pause
