@echo off
setlocal
chcp 65001 >nul
rem The Leyline Discord redo, done: backs up the server to your Desktop first, then changes it.
rem Only Control's discord-apply step can run it (after you type YES in Control under the preview); double-clicked it stops.
rem Leyline Control runs this (its Discord panel); it never asks anything. The bot token comes from token.txt next to
rem this file, which Control's "Discord bot token" box saves (git-ignored, never printed or logged).
cd /d "%~dp0"
set "PYTHONIOENCODING=utf-8"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo Python 3 is needed: https://www.python.org/downloads/  ^(tick "Add python.exe to PATH"^)
  exit /b 1
)
%PY% discord_leyline.py --yes-from-control
exit /b %ERRORLEVEL%
