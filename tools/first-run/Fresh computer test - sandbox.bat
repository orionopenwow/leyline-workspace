@echo off
setlocal
title Leyline - fresh computer test in Windows Sandbox
rem The fresh computer test in a brand-new Windows (docs/FRESH-COMPUTER-TEST.md part C): writes
rem tools\first-run\logs\sandbox\Leyline fresh computer.wsb and opens Windows Sandbox on it. Inside, the test copies
rem your game folder (mapped read-only, so nothing of yours changes) and runs by itself with this PC's Python; its
rem report lands in tools\first-run\logs\sandbox\out\logs\first-run-latest.txt. Close the sandbox window when it is done:
rem everything in it is thrown away. Needs Windows Sandbox on (Windows Features; Pro, Enterprise or Education).
rem   Fresh computer test - sandbox.bat             with your Wrath folder
rem   Fresh computer test - sandbox.bat --no-game   without a game folder (quicker)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\first_run.py" sandbox --open %*
exit /b %ERRORLEVEL%
