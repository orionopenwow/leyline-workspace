@echo off
setlocal
title Leyline - Vanilla Remastered game data (check, read-only)
rem Read-only: finds this PC's 1.12.1 Data folder and its archives, and asks the server what is there (disk, memory,
rem the world server's data, the slice, the extractors in the realm's images, which phases are done). Changes nothing.
cd /d "%~dp0"
if not exist vr-data.ps1 (echo vr-data.ps1 is missing next to this file. & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy Bypass -File vr-data.ps1 -Data "%~1" -CheckOnly
if errorlevel 1 (echo. & echo The check did not pass; see above. & exit /b 1)
exit /b 0
