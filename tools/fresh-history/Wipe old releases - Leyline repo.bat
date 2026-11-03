@echo off
rem Old launcher releases off the public Leyline repo: every launcher-v* release and tag except the newest.
rem Owner 2 Nov 22:36: "wipe releases up until our last one now". launcher-latest, host-pack, host-kit and
rem leyline-addons stay (the launcher and the site download from them).
setlocal
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed. & exit /b 1)
if /i "%~1"=="check" (%PY% "tools\fresh-history\wipe_releases.py" --repo orionopenwow/Leyline) else (%PY% "tools\fresh-history\wipe_releases.py" --repo orionopenwow/Leyline --go)
exit /b %errorlevel%
