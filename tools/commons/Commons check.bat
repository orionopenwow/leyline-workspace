@echo off
rem Leyline Control step "commons-check": the commons kit checks itself (read-only: makes a sample network in a temp folder).
rem tools/commons/README.md, docs/BLANK-SLATE.md. The directory (directory.json), then the kit's tests.
setlocal
cd /d "%~dp0..\.."
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -I "tools\commons\commons.py" check-directory || exit /b 1
%PY% -I "tools\commons\test_commons.py"
exit /b %ERRORLEVEL%
