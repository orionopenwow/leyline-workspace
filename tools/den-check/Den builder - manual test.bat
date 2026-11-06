@echo off
setlocal
title Leyline - Den builder, your manual pass
rem Owner 4 Nov: the Den builder tried by hand. Starts Your Den the way the My Den button does (the game signs in by
rem itself and stays open), then shows each check here: try it in the game, press Enter if it works, n if not.
rem /wudebug in the game shows the editor's state and Den Lite's (the ray, the ground hit, the redraw pace).
rem Your answers and what Den Lite heard: tools\den-check\logs\den-manual-latest.txt
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & pause & exit /b 1)
%PY% -I "tools\den-check\den_check.py" manual --timeout 180
echo.
echo Send tools\den-check\logs\den-manual-latest.txt when done.
pause
exit /b 0
