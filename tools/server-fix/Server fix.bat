@echo off
setlocal
title Leyline - server fix (folder rights, .bug look)
rem Makes the streamed-file folders writable by root only, and looks (read only) at why .bug did not answer.
rem No restart. Output: live\fix-<date>.txt
cd /d "%~dp0"
if not exist fix.sh (echo fix.sh is missing next to this file. & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\fix-%D%.txt"
type fix.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/wu-fix.sh && bash /tmp/wu-fix.sh; rm -f /tmp/wu-fix.sh" > "%OUT%"
type "%OUT%"
findstr /c:"SERVER FIX DONE" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
exit /b 0
