@echo off
setlocal
title Azeroth Survivors - a local preview for 20 minutes (127.0.0.1:18093)
rem 2 Nov (owner: "a small gif of Azeroth Survivors gameplay for the site and launcher card"): the game on this PC only,
rem from your own 1.12 folder, at http://127.0.0.1:18093/ for 20 minutes, so Claude can film it in the Leyline
rem browser pane. Nothing leaves this PC; the host stops by itself.
cd /d "%~dp0..\.."
set "TS=tools\survivors"
set "OUT=%TS%\out"
if not exist "%OUT%\data-folder.txt" (echo No %OUT%\data-folder.txt: record first. & exit /b 1)
set /p DATA=<"%OUT%\data-folder.txt"
set "DATA=%DATA:"=%"
set "HOSTEXE=%TS%\bin\wenilla-host.exe"
if not exist "%HOSTEXE%" (echo %HOSTEXE% is missing. & exit /b 1)
start "Survivors preview host - stops by itself" /min "%HOSTEXE%" --www "%TS%\dist" --data "%DATA%" --bind 127.0.0.1:18093
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -I "%TS%\survivors_rec.py" --wait-only 120 --upstream http://127.0.0.1:18093 || (echo The host did not start. & exit /b 1)
echo PREVIEW UP: http://127.0.0.1:18093/ (20 minutes)
powershell -NoProfile -Command "Start-Sleep -Seconds 1200"
for /f "tokens=2" %%p in ('tasklist /fi "windowtitle eq Survivors preview host*" /fo list ^| find "PID:"') do taskkill /pid %%p /f >nul 2>&1
echo PREVIEW DONE
exit /b 0
