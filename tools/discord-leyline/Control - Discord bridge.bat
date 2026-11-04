@echo off
setlocal
chcp 65001 >nul
rem Links the game's World chat to Discord: finds #world-chat with the saved bot token (token.txt, never printed),
rem installs the bridge on the server over one SSH login and turns it on. Last line: DISCORD LINK OK.
cd /d "%~dp0"
set "PYTHONIOENCODING=utf-8"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo Python 3 is needed: https://www.python.org/downloads/  ^(tick "Add python.exe to PATH"^)
  exit /b 1
)
%PY% bridge_link.py --from-control %*
exit /b %ERRORLEVEL%
