@echo off
setlocal
chcp 65001 >nul
rem Read-only: the newest messages in every Discord channel since the last read, into live\inbox-latest.md (on this PC
rem only). Posts, edits and deletes nothing. The bot token comes from token.txt (Control's Discord card; never printed).
cd /d "%~dp0"
set "PYTHONIOENCODING=utf-8"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo Python 3 is needed: https://www.python.org/downloads/  ^(tick "Add python.exe to PATH"^)
  exit /b 1
)
%PY% discord_read.py --from-control %*
exit /b %ERRORLEVEL%
