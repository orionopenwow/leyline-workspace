@echo off
setlocal
title Leyline - Sky Island (on this PC)
rem Sky Island on this PC (owner 5 Nov: "a local version for me"): the smoke with no game window first, then the
rem island's file in Your Den, and My Den started as its button does: the game signs in by itself and you land on the
rem island. Your own game's models; nothing goes to GitHub or any server.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\den-check\sky_island.py" %*
exit /b %ERRORLEVEL%
