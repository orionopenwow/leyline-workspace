@echo off
setlocal
title Leyline - the newest in-game UI into this PC's game (for testing)
rem tools\den-check\ui_local.py: copies stream\fui-web addons into the Den's game folder, backup first; "--undo" puts it back.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed. & exit /b 1)
%PY% -I "tools\den-check\ui_local.py" %*
exit /b %ERRORLEVEL%
