@echo off
setlocal
title Leyline - Azeroth Survivors: record its game files (on this PC only)
rem Owner-run, on this PC, from your own 1.12 client (docs\SURVIVORS.md, "Make the game files"). Nothing is uploaded.
rem   1. checks the browser build in tools\survivors\dist against stream\survivors\upstream.lock.json
rem   2. runs the upstream game host on this PC (127.0.0.1:18090) on your client's Data folder, and the recorder in
rem      front of it (127.0.0.1:18091): a fresh Edge window plays the game's own autopilot zone by zone, class by
rem      class (about 45 minutes; keep the window visible), then asks for one pass by hand on the pick screen
rem   3. exports exactly the files the game asked for into tools\survivors\out\slice and writes names.sha256
rem   4. survivors-check: the slice against names.sha256
rem Then: "Record Survivors files.bat verify" plays the slice alone, through the same host the server runs, and
rem lists anything still missing (the next record adds it). Then hosting\survivors\Put Survivors on Leyline.bat.
rem   Record Survivors files.bat [<Data folder>]     record (the full plan)
rem   Record Survivors files.bat quick [<Data>]      a short plan (two zones, two classes) to try it out
rem   Record Survivors files.bat verify              the slice alone, a short plan, gaps to out\verify\missing.txt
rem Needs Python 3.9+ and Edge (or Chrome). The browser build and the upstream host come from this repo's GitHub
rem release survivors-latest (made by the survivors workflow), downloaded and checked hash by hash into
rem tools\survivors\dist and tools\survivors\bin (survivors_fetch.py; nothing to build). Safe to run again: names add up.
cd /d "%~dp0..\.."
set "TS=tools\survivors"
set "OUT=%TS%\out"
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)" >nul 2>&1 || (echo Python 3.9 or newer is needed: https://www.python.org/downloads/ & pause & exit /b 1)
set "MODE=record"
set "PLAN=full"
if /i "%~1"=="quick" (set "PLAN=quick" & shift)
if /i "%~1"=="verify" (set "MODE=verify" & set "PLAN=quick" & shift)
set "DIST=%TS%\dist"
echo == the locked browser build and the upstream host (downloaded once, every file checked)
%PY% -I "%TS%\survivors_fetch.py"
if errorlevel 1 (echo. & echo Could not get the locked build ^(see above^). & pause & exit /b 1)
%PY% -I "%TS%\survivors_check.py" dist "%DIST%"
if errorlevel 1 (echo. & echo The browser build in %DIST% is not the locked one ^(see above^). & pause & exit /b 1)
if not exist "%OUT%" mkdir "%OUT%"
if "%MODE%"=="verify" goto verify

set "DATA=%~1"
if "%DATA%"=="" if exist "%OUT%\data-folder.txt" set /p DATA=<"%OUT%\data-folder.txt"
if "%DATA%"=="" set /p "DATA=Your 1.12 client's Data folder (drag it into this window, then Enter): "
set "DATA=%DATA:"=%"
if not exist "%DATA%\*.MPQ" (echo No .MPQ files in "%DATA%": that is not a client's Data folder. & pause & exit /b 1)
>"%OUT%\data-folder.txt" echo %DATA%
set "HOSTEXE=%TS%\bin\wenilla-host.exe"
if not exist "%HOSTEXE%" (echo %HOSTEXE% is missing ^(the download above should have put it there^). & pause & exit /b 1)
set "HOSTARGS=--www "%DIST%" --data "%DATA%""
set "REC=%OUT%"
goto run

:verify
if not exist "%OUT%\slice" (echo No slice yet: run a record first. & pause & exit /b 1)
%PY% -I "%TS%\survivors_check.py" slice "%OUT%\slice" "%OUT%\names.sha256" || (pause & exit /b 1)
set "HOSTEXE=stream\bin\wenilla-host.exe"
if not exist "%HOSTEXE%" (echo The Leyline host stream\bin\wenilla-host.exe is not on this PC: the verify pass needs it. & pause & exit /b 1)
set "HOSTARGS=--www "%DIST%" --patch "%OUT%\slice" --only-listed"
set "REC=%OUT%\verify"
if exist "%REC%" rmdir /s /q "%REC%"
mkdir "%REC%"

:run
taskkill /im wenilla-host.exe /f >nul 2>&1
echo == the game host on this PC (127.0.0.1:18090)
start "Survivors game host - closes by itself" /min "%HOSTEXE%" %HOSTARGS% --bind 127.0.0.1:18090
%PY% -I "%TS%\survivors_rec.py" --wait-only 120 --upstream http://127.0.0.1:18090
if errorlevel 1 (echo The game host did not start ^(see its window^). & taskkill /im wenilla-host.exe /f >nul 2>&1 & pause & exit /b 1)
echo == the recorder: a new Edge window opens. Keep it visible until it says done.
set "URL=http://127.0.0.1:18091/__rec/"
set BR="--user-data-dir=%CD%\%OUT%\browser" --no-first-run --disable-renderer-backgrounding --disable-background-timer-throttling --new-window
if exist "%OUT%\browser" rmdir /s /q "%OUT%\browser"
start "" msedge %BR% "%URL%" 2>nul || start "" chrome %BR% "%URL%" 2>nul || start "" "%URL%"
%PY% -I "%TS%\survivors_rec.py" --upstream http://127.0.0.1:18090 --listen 127.0.0.1:18091 --out "%REC%" --plan %PLAN%
if errorlevel 1 (taskkill /im wenilla-host.exe /f >nul 2>&1 & pause & exit /b 1)
if "%MODE%"=="verify" goto verified

echo == export: the recorded files out of your client, into %OUT%\slice
set "LISTS="%OUT%\recorded.txt""
if exist "%OUT%\verify\missing.txt" set "LISTS=%LISTS% "%OUT%\verify\missing.txt""
%PY% -I "%TS%\survivors_export.py" %LISTS% --out "%OUT%" --host http://127.0.0.1:18090
set "RC=%ERRORLEVEL%"
taskkill /im wenilla-host.exe /f >nul 2>&1
if not "%RC%"=="0" (echo The export stopped ^(see above^). & pause & exit /b 1)
%PY% -I "%TS%\survivors_check.py" slice "%OUT%\slice" "%OUT%\names.sha256"
if errorlevel 1 (pause & exit /b 1)
echo.
echo Done: %OUT%\slice and %OUT%\names.sha256. Next: "Record Survivors files.bat verify", then
echo hosting\survivors\Put Survivors on Leyline.bat
pause
exit /b 0

:verified
taskkill /im wenilla-host.exe /f >nul 2>&1
set "N=0"
if exist "%REC%\missing.txt" for /f %%c in ('type "%REC%\missing.txt" ^| find /c /v ""') do set "N=%%c"
echo.
if "%N%"=="0" (echo Verified: the slice alone held every file the game asked for.) else (echo %N% file^(s^) the game asked for are not in the slice: %REC%\missing.txt. Run a record again ^(it adds them^), then verify again. The franchise logo is left out on purpose.)
pause
exit /b 0
