@echo off
rem Release tags of the public Leyline repo that still point at old commits by someone else, moved onto Orion's
rem main (retag.py). Releases and their files stay. "check": a dry run. Owner 3 Nov: scrub chumpton from Leyline.
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
set "CF=tools\fresh-history\CONFIRM-retag"
if not exist "%CF%" if exist "%CF%.txt" set "CF=%CF%.txt"
if /i "%~1"=="check" (%PY% "tools\fresh-history\retag.py" --root "%LY%") else (%PY% "tools\fresh-history\retag.py" --go --root "%LY%" --confirm "%CF%")
exit /b %errorlevel%
