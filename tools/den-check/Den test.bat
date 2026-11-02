@echo off
setlocal
title Leyline - den-test (starts your game by itself)
rem The real My Den, unattended: the launcher's own My Den code (Leyline.exe den-play --test) starts Den Lite and
rem your game, signs in, waits for the white island, takes a screenshot, then closes the game and Den Lite and takes
rem the one-time sign-in out. PASS / FAIL per step with times. Logs: tools\den-check\logs\den-test-latest.txt and .png
rem   Den test.bat            the Den alone
rem   Den test.bat invite     also a friend (a test login through the friends' listener) joins, and your game sees them
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
if /i "%~1"=="invite" (%PY% -I "tools\den-check\den_check.py" test --invite) else (%PY% -I "tools\den-check\den_check.py" test)
exit /b %ERRORLEVEL%
