@echo off
setlocal
title Leyline - bug manifest
rem Every bug report (game, site, launcher, Discord) in one de-duplicated list: the server rebuilds it, this copies
rem BUGS-MANIFEST.md and backlog.json into tools\bugs\live\ (not in git: player text stays off GitHub).
rem tools\bugs\status.json (statuses set by a Claude session or by hand) goes up first. docs\BUGS-MANIFEST.md.
cd /d "%~dp0"
if not exist bugs-pull.sh (echo bugs-pull.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
set "KEY=%USERPROFILE%\.ssh\openwow"
set "HOST=root@play.openvanilla.app"
set "SSHO=-o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30"
if exist status.json (
  type status.json | ssh -i "%KEY%" %SSHO% %HOST% "install -d -m 755 /srv/wrath/bugs && cat > /srv/wrath/bugs/status.json.new" || (echo Could not upload status.json. & exit /b 1)
)
type bugs-pull.sh | ssh -i "%KEY%" %SSHO% %HOST% "tr -d '\r' > /tmp/wu-bugs.sh && bash /tmp/wu-bugs.sh; rm -f /tmp/wu-bugs.sh" > live\pull.txt
type live\pull.txt
findstr /b /c:"BUGS OK" live\pull.txt >nul || (echo It did not finish; see tools\bugs\live\pull.txt. & exit /b 1)
scp -q -i "%KEY%" %SSHO% %HOST%:/srv/wrath/bugs/BUGS-MANIFEST.md %HOST%:/srv/wrath/bugs/backlog.json live\ || (echo Could not copy the files. & exit /b 1)
echo Saved tools\bugs\live\BUGS-MANIFEST.md and backlog.json
exit /b 0
