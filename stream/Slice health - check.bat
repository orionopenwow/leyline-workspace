@echo off
setlocal
title Leyline - is the streamed slice whole? (read-only)
rem Read-only: every slice folder on the server (slices\<name>, and any .old/.new/.stage/.prev copy), its file count
rem per folder, the key files (DBFilesClient\Spell.dbc, Map.dbc, AreaTable.dbc, Interface\GlueXML\GlueXML.toc) and
rem how many of the names in its own tier lists (.tiers) are on disk; then what wu-play serves and what it answers.
rem Changes nothing. Saved to logs\slice-health-latest.txt. One SSH login.
rem PROBLEM lines say what is wrong; the last line is "SLICE HEALTH DONE: OK" or "SLICE HEALTH DONE: PROBLEMS".
rem After a bad slice: bash /srv/wrath/stream/slice-install.sh --rollback mini   (when slices\mini.old is whole),
rem or run "Make mini slice.bat" again. docs\INCIDENT-2026-11-01-SLICE.md.
cd /d "%~dp0"
if not exist server\slice-health.sh (echo server\slice-health.sh is missing. & pause & exit /b 1)
if not exist "..\logs" mkdir "..\logs"
set "OUT=..\logs\slice-health-latest.txt"
ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "sed 's/\r$//' > /tmp/wu-slice-health.sh && bash /tmp/wu-slice-health.sh 2>&1; rm -f /tmp/wu-slice-health.sh" < server\slice-health.sh > "%OUT%"
type "%OUT%"
findstr /b /c:"SLICE HEALTH DONE" "%OUT%" >nul || (echo. & echo The check did not finish; see above. & pause & exit /b 1)
findstr /b /c:"SLICE HEALTH DONE: OK" "%OUT%" >nul || (echo. & echo The slice has problems: see the PROBLEM lines above. & pause & exit /b 2)
pause
exit /b 0
