@echo off
setlocal
title Leyline - make room on the server (dry run)
rem Dry run: what the tidy would remove on the server (build cache, old logs, old site backups, the parked Vanilla
rem Remastered browser slice) and how much it frees. Changes nothing. Output: tools\server-tidy\live\tidy-<date>.txt
cd /d "%~dp0"
if not exist tidy.sh (echo tidy.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\tidy-dry-%D%.txt"
type tidy.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/wu-tidy.sh && bash /tmp/wu-tidy.sh; rm -f /tmp/wu-tidy.sh" > "%OUT%"
type "%OUT%"
findstr /c:"SERVER TIDY DRY RUN OK" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
exit /b 0
