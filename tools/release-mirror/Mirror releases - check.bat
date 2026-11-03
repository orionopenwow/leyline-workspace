@echo off
rem Leyline Control step "release-mirror-check": what "Mirror releases.bat" would copy (read-only).
setlocal
cd /d "%~dp0..\.."
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -I "tools\release-mirror\release_mirror.py" check
exit /b %ERRORLEVEL%
