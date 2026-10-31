@echo off
setlocal
title Leyline - where the old 1.12 game files are (read-only)
rem Read-only: lists every folder of 1.12 game data on the server, its size, the container that mounts it and which
rem sites serve /data, into tools\ov112-data\live\data-check-<date>.txt (not in git). Changes nothing.
cd /d "%~dp0"
if not exist data-check.sh (echo data-check.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\data-check-%D%.txt"
type data-check.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/ov112-data-check.sh && bash /tmp/ov112-data-check.sh; rm -f /tmp/ov112-data-check.sh" > "%OUT%"
findstr /b /c:"OV112 DATA CHECK OK" "%OUT%" >nul || (echo It did not finish; see %OUT%. & pause & exit /b 1)
echo Saved %OUT%
exit /b 0
