@echo off
setlocal
chcp 65001 >nul
rem Posts ONE announcement draft to #announcements. Only Control's discord-announce step can run it: after the owner
rem read the draft in Control > Discord > Pending announcements and typed YES there (double-clicked it stops).
rem The bot token comes from token.txt next to this file (Control's Discord card saves it; never printed or logged).
cd /d "%~dp0"
set "PYTHONIOENCODING=utf-8"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo Python 3 is needed: https://www.python.org/downloads/  ^(tick "Add python.exe to PATH"^)
  exit /b 1
)
%PY% announce.py post --yes-from-control
exit /b %ERRORLEVEL%
