@echo off
setlocal
title Leyline - first-run-test (a brand-new launcher, through to in the game)
rem The first-run test (docs/FIRST-RUN-TEST.md): today's launcher download in an empty data folder (nothing of your own
rem launcher is used or changed), your Wrath folder added, a test Hero Key logged in, PLAY on Wrath Unleashed, the game
rem logs in by itself, a test character into the world, screenshots, then the Den test on the same fresh launcher. The
rem test account is closed and the temp folders removed after, always; your game folder's files are put back.
rem PASS / FAIL per phase with the fix. Logs: tools\first-run\logs\first-run-latest.txt and the .png files
rem   First run test.bat            all of it
rem   First run test.bat --no-den   without the Den test
rem   First run test.bat --keep     keep the fresh folders (to open that launcher by hand: docs/FIRST-RUN-TEST.md 3.6)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\first_run.py" %*
exit /b %ERRORLEVEL%
