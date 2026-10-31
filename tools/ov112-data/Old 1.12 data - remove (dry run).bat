@echo off
setlocal
title Leyline - remove the old 1.12 game files (dry run)
rem Owner 2026-10-31: "remove old game files, keep new slice". Runs data-remove.sh on the server: the old play site
rem answers 410 for /data, then the old 1.12 client archives and extracted maps are emptied (the Wrath stream and the
rem Wrath server's data are never touched). Log: tools\ov112-data\live\data-remove-<date>.txt.
cd /d "%~dp0"
if not exist data-remove.sh (echo data-remove.sh is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\data-remove-dry-%D%.txt"
type data-remove.sh | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "tr -d '\r' > /tmp/ov112-data-remove.sh && bash /tmp/ov112-data-remove.sh; rm -f /tmp/ov112-data-remove.sh" > "%OUT%"
type "%OUT%"
findstr /b /c:"OV112 DATA REMOVE DRY" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
exit /b 0
