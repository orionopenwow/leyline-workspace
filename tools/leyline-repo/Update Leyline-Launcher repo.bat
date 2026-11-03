@echo off
rem The old Open Vanilla repo, now github.com/orionopenwow/Leyline-Launcher, gets the same Leyline version as
rem github.com/orionopenwow/Leyline (owner 2 Nov). The same export and checks as "Update Leyline repo.bat", into
rem .repos\leyline-launcher, committed as Orion. Kept as they are in that repo: www/ (the openvanilla.app site and the
rem old launchers' update files), its Pages and uptime workflows and the two files the Pages job signs with. Its
rem tags and releases are never touched; never force-pushes.
setlocal
set "LEYLINE_SLUG=orionopenwow/Leyline-Launcher"
set "LEYLINE_REPO_DIR=%~dp0..\..\.repos\leyline-launcher"
set "LEYLINE_KEEP=www .github/workflows/pages.yml .github/workflows/uptime.yml packaging/live_content.py launcher/ed25519.py launcher/live_content.py"
call "%~dp0Update Leyline repo.bat"
exit /b %errorlevel%
