@echo off
setlocal
title Leyline - make room on the server
rem Removes (owner 1 Nov 2026): Docker's build cache, system logs older than 7 days, all but the newest 3 site-update
rem backups, and the parked Vanilla Remastered browser slice (/srv/vr112/slice). Nothing else. Output: live\tidy-<date>.txt
cd /d "%~dp0"
if not exist tidy.sh (echo tidy.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\tidy-%D%.txt"
type tidy.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/wu-tidy.sh && bash /tmp/wu-tidy.sh --go; rm -f /tmp/wu-tidy.sh" > "%OUT%"
type "%OUT%"
findstr /c:"SERVER TIDY DONE" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
exit /b 0
