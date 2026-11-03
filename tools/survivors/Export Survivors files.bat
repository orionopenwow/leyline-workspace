@echo off
setlocal
title Azeroth Survivors - export the recorded files (no new recording)
rem 2 Nov: the recording finished but its export never ran (the step started itself over). This does only the export:
rem the upstream host on this PC (127.0.0.1:18092, a port the recorder doesn't use) on your client's Data folder, then
rem the files the game asked for (out\recorded.txt) copied into out\slice with out\names.sha256, then checked.
cd /d "%~dp0..\.."
set "TS=tools\survivors"
set "OUT=%TS%\out"
set PY=py -3
where py >nul 2>&1 || set PY=python
if not exist "%OUT%\recorded.txt" (echo No %OUT%\recorded.txt: record first. & exit /b 1)
if not exist "%OUT%\data-folder.txt" (echo No %OUT%\data-folder.txt: record first. & exit /b 1)
set /p DATA=<"%OUT%\data-folder.txt"
set "DATA=%DATA:"=%"
if not exist "%DATA%\*.MPQ" (echo No .MPQ files in "%DATA%". & exit /b 1)
set "HOSTEXE=%TS%\bin\wenilla-host.exe"
if not exist "%HOSTEXE%" (echo %HOSTEXE% is missing. & exit /b 1)
echo == the game host on this PC (127.0.0.1:18092)
start "Survivors export host - closes by itself" /min "%HOSTEXE%" --www "%TS%\dist" --data "%DATA%" --bind 127.0.0.1:18092
%PY% -I "%TS%\survivors_rec.py" --wait-only 120 --upstream http://127.0.0.1:18092
if errorlevel 1 (echo The game host did not start. & exit /b 1)
echo == export: the recorded files out of your client, into %OUT%\slice
set "LISTS="%OUT%\recorded.txt""
if exist "%OUT%\verify\missing.txt" set "LISTS=%LISTS% "%OUT%\verify\missing.txt""
%PY% -I "%TS%\survivors_export.py" %LISTS% --out "%OUT%" --host http://127.0.0.1:18092
set "RC=%ERRORLEVEL%"
for /f "tokens=2" %%p in ('tasklist /fi "windowtitle eq Survivors export host*" /fo list ^| find "PID:"') do taskkill /pid %%p /f >nul 2>&1
if not "%RC%"=="0" (echo The export stopped ^(see above^). & exit /b 1)
%PY% -I "%TS%\survivors_check.py" slice "%OUT%\slice" "%OUT%\names.sha256" || exit /b 1
echo EXPORT OK: %OUT%\slice and %OUT%\names.sha256
exit /b 0
