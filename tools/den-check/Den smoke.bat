@echo off
setlocal
title Leyline - Den smoke (no game window)
rem The Den's protocol-level smoke (docs/DEN-NEXT.md): this PC's Den Lite on spare ports with a scratch folder and the
rem headless client: the way in, Sky Island, a fall off, a placement, the mouse against the model, the island as a
rem code, home. Needs no game and no window. Logs: tools\den-check\logs\den-smoke-latest.txt
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\den-check\den_smoke.py"
exit /b %ERRORLEVEL%
