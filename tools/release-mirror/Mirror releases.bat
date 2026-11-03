@echo off
rem Leyline Control step "release-mirror" (docs/RESILIENCE.md): after a ship, copy each Leyline release (installers,
rem latest.json), the Host pack, the Leyline addons and the upstream server source at our pinned commits from GitHub to
rem our server (https://play.openvanilla.app/wrath/leyline/releases/), with checksums and torrents, so installed
rem launchers keep updating and people can keep hosting if GitHub is down. With the release key on this PC
rem (%USERPROFILE%\.leyline\release-log.key) it also signs the new entries of the release log. Only new files are
rem uploaded (one SSH login); the server keeps a backup of its lists and puts them back on failure. Safe to run again.
setlocal
cd /d "%~dp0..\.."
set "KEY=%USERPROFILE%\.ssh\openwow"
if not exist "%KEY%" (echo SSH key %KEY% not found. & exit /b 1)
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)" >nul 2>&1 || (echo Python 3.9 or newer is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\release-mirror\release_mirror.py" upload --ssh-key "%KEY%" %*
exit /b %ERRORLEVEL%
