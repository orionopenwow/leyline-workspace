@echo off
setlocal
chcp 65001 >nul
rem Read-only: checks every announcement draft in announce\pending (length, brand words, links, ping rules).
rem Posts nothing and needs no token, so Autopilot may run it.
cd /d "%~dp0"
set "PYTHONIOENCODING=utf-8"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo Python 3 is needed: https://www.python.org/downloads/  ^(tick "Add python.exe to PATH"^)
  exit /b 1
)
%PY% announce.py check --all
exit /b %ERRORLEVEL%
