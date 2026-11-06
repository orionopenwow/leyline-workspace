@echo off
setlocal
title Leyline - your Den with Project Reforged
rem Your Den with Project Reforged's graphics, and only your Den: the patches go into your game's Data folder, your Den
rem starts (the My Den button's way), and when the game closes they come out again, so the Official and Community
rem realms keep the normal look. Keep this window open while you play. If it was closed early, run
rem "Den looks - take Project Reforged out.bat".
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & pause & exit /b 1)
%PY% -I "tools\den-looks\den_looks.py" play --with "%~1"
echo.
pause
exit /b %ERRORLEVEL%
