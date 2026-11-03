@echo off
setlocal
title Leyline - quick apply (check, read-only)
rem Quick Claude edits (tools/control/quick.py): control\quick\<id>.json says which files and which deploy step;
rem every file is checked (inside Leyline, no secrets, its sha256) before anything runs. Control passes the id.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is missing: install it from python.org. & exit /b 1)
%PY% -I tools\control\quick.py check
exit /b %ERRORLEVEL%
