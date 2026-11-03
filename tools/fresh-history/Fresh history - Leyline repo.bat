@echo off
rem The public Leyline repo (github.com/orionopenwow/Leyline): main replaced with one commit as Orion, the same
rem files; tags and releases stay (the launcher downloads hang on them). Backup bundle on the Desktop first.
rem Owner 2 Nov: "Leyline and Leyline Launcher are by me Orion, so wipe chumpton as contributor".
setlocal
cd /d "%~dp0..\.."
set "WU=%CD%"
set "LY=%WU%\.repos\leyline-public"
if not exist "%LY%\.git" if exist "%USERPROFILE%\Desktop\Leyline\.git" if /i not "%USERPROFILE%\Desktop\Leyline"=="%WU%" set "LY=%USERPROFILE%\Desktop\Leyline"
if not "%LEYLINE_REPO_DIR%"=="" set "LY=%LEYLINE_REPO_DIR%"
if not exist "%LY%\.git" (echo No Leyline repo folder at %LY%: run the leyline step once first. & exit /b 1)
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed. & exit /b 1)
rem Notepad saves "CONFIRM-leyline.txt" (Windows hides the .txt): either name works
set "CF=tools\fresh-history\CONFIRM-leyline"
if not exist "%CF%" if exist "%CF%.txt" set "CF=%CF%.txt"
set "ARGS=--repo orionopenwow/Leyline --root "%LY%" --confirm "%CF%""
if /i "%~1"=="check" (%PY% "tools\fresh-history\fresh_history.py" %ARGS%) else (%PY% "tools\fresh-history\fresh_history.py" --go %ARGS%)
exit /b %errorlevel%
