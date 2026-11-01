@echo off
setlocal
title Leyline - Vanilla Remastered game data (upload, extract, 1-20 browser slice)
rem Owner 2026-10-31. From this PC's own 1.12.1 client: uploads its archives to the server (resumable, checked),
rem extracts the world server's data there, makes the levels 1-20 browser slice, sets the realm up compact, then
rem deletes the uploaded archives. Long (upload time + about 45-90 min on the server). Running it again goes on
rem where it stopped. Optional: the 1.12.1 Data folder as the first argument. Read-only first: "... - check.bat".
cd /d "%~dp0"
if not exist vr-data.ps1 (echo vr-data.ps1 is missing next to this file. & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy Bypass -File vr-data.ps1 -Data "%~1"
if errorlevel 1 (echo. & echo It did not finish; see above. Run it again to go on. & exit /b 1)
exit /b 0
