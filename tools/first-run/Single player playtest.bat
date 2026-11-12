@echo off
setlocal
title Leyline - sp-playtest (single player, nobody at the keyboard)
rem The single player playtest (tools\first-run\sp_playtest.py): the launcher-test build (checked against its .sha256)
rem in a brand-new data folder (never your own: it refuses your launcher's and Solo's folders), your Wrath game folder
rem from your launcher, a test Hero Key when the website gives one; then Single player in one click: setup, Play with a
rem random Quick start, into the world, companions in the group, levelling to 10 by the world's own console on this
rem local world (server-driven, not hand play), the game closed and the world stopping by itself, a deleted engine file
rem put back by Repair, Play again. Screenshots at each phase. After, always: the test's world stopped, your game
rem folder put back, the test account closed, the temp folder removed. Stop your own Single player world first.
rem Results: logs\sp-playtest\<time>\ and logs\sp-playtest\latest.txt. Up to 4 hours (the first setup reads the maps).
rem   Single player playtest.bat            Single player, every check
rem   Single player playtest.bat --both     then the original world too
rem   Single player playtest.bat --no-key   without a test Hero Key (the offline login)
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\sp_playtest.py" run %*
exit /b %ERRORLEVEL%
