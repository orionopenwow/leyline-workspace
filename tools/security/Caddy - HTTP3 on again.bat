@echo off
setlocal
title Leyline - HTTP/3 on again (undo h3-off)
rem HTTP/3 on again (undo h3-off). Runs h3-off.sh --undo on the server (root@play.openvanilla.app, %USERPROFILE%\.ssh\openwow), one SSH login.
rem Backs up the Caddyfile to /root/caddy-backups/<time>-h3off, validates, reloads, self-checks; puts the old file back if anything fails.
cd /d "%~dp0"
if not exist h3-off.sh (echo h3-off.sh is missing next to this file. & pause & exit /b 1)
set "OUT=%TEMP%\leyline-h3.txt"
type h3-off.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/h3-off.sh && bash /tmp/h3-off.sh --undo; rc=$?; rm -f /tmp/h3-off.sh; exit $rc" > "%OUT%" 2>&1
set RC=%ERRORLEVEL%
type "%OUT%"
findstr /c:"H3 UNDO OK" "%OUT%" >nul || (echo It did not finish. & exit /b 1)
exit /b %RC%
