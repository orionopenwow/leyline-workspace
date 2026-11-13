@echo off
setlocal
title Leyline - sp-playtest-boss (Boss Rush, a full boss fight, nobody at the keyboard)
rem Boss Rush's playtest (tools\first-run\sp_playtest.py --bossrush): the launcher-test build (checked against its
rem .sha256 and for the Boss Rush playtest) in a brand-new data folder (never your own), your Wrath game folder from
rem your launcher; then Boss Rush set up from it, Start in front of the final fight with a full companion raid (the
rem raid's place, its instance and its members checked on the realm itself), and the whole fight run to its end with
rem your hero on the bots' own AI: its start, its phase changes, a kill or a wipe (said as it was), then Retry back in
rem front of the boss. Screenshots at each phase. After, always: the test's realm stopped, your game folder put back,
rem the temp folder removed. Stop your own Single player world and Boss Rush first (the same ports).
rem Results: logs\boss-playtest\<time>\ and logs\boss-playtest\latest.txt. Up to 4 hours (the first setup reads the maps).
rem   Boss Rush playtest.bat                              the final fight, 10 players, normal
rem   Boss Rush playtest.bat --boss icc-plague            a wing (or any boss; icc-raid for the whole raid: bossstart only)
rem   Boss Rush playtest.bat --size 25 --diff heroic --hero paladin-holy
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\sp_playtest.py" run --bossrush %*
exit /b %ERRORLEVEL%
