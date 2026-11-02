@echo off
setlocal
title Leyline - den-check (read-only)
rem Read-only: is My Den one click into the world on this PC? The game folder the launcher plays Your Den with,
rem its archives, Leyline's login archive, Forever UI, Config.wtf, Den Lite, and a sign-in through the exact steps
rem the game takes (on a Den Lite of its own, spare ports, a scratch folder). PASS / FAIL per step and the one fix.
rem   Den check.bat [game folder]
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
if "%~1"=="" (%PY% -I "tools\den-check\den_check.py" check) else (%PY% -I "tools\den-check\den_check.py" check --game "%~1")
exit /b %ERRORLEVEL%
