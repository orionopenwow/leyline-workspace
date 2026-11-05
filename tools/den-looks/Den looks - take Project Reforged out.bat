@echo off
setlocal
title Leyline - take Project Reforged out of the game folder
rem Takes out exactly the Project Reforged patches the Den put in (after a closed window or a crash). Nothing else.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & pause & exit /b 1)
%PY% -I "tools\den-looks\den_looks.py" clean
%PY% -I "tools\den-looks\den_looks.py" status
echo.
pause
exit /b 0
