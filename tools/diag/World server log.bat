@echo off
setlocal
title Leyline - why did the world server restart (read-only)
rem Read-only: the world server's state, restart count, recent output and crash dumps, into
rem tools\diag\live\worldlog-<date>.txt (not in git). Changes nothing.
cd /d "%~dp0"
if not exist worldlog.sh (echo worldlog.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\worldlog-%D%.txt"
type worldlog.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/wu-worldlog.sh && bash /tmp/wu-worldlog.sh; rm -f /tmp/wu-worldlog.sh" > "%OUT%"
findstr /b /c:"WORLDLOG OK" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
echo Saved %OUT%
exit /b 0
