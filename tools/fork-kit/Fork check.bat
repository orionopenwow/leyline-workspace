@echo off
rem Leyline Control step "fork-check": is this copy ready to run under another name, keys and servers? (read-only)
rem docs/TOOLKIT.md and docs/SUCCESSION.md; tools/fork-kit/fork_kit.py. Fetches the update sources, mirrors and relays.
setlocal
cd /d "%~dp0..\.."
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -I "tools\fork-kit\fork_kit.py" check --online
exit /b %ERRORLEVEL%
