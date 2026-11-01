@echo off
setlocal
title Leyline - save to GitHub again (fresh commit)
rem When GitHub refused a save (its secret scanning blocked a commit, 1 Nov), the refused commit stays on this PC
rem and every later save is refused with it. This folds the saves GitHub hasn't got yet back into the working folder
rem (git reset --soft to GitHub's main: no file changes, nothing deleted) and runs "Ship to GitHub.bat" once more,
rem which makes one fresh commit from the files as they are now.
cd /d "%~dp0..\.." || exit /b 1
where git >nul 2>&1 || (echo Git is not installed. & exit /b 1)
git fetch -q origin || (echo Could not reach GitHub. & exit /b 1)
for /f %%N in ('git rev-list --count origin/main..HEAD') do set "AHEAD=%%N"
echo Saves on this PC that GitHub has not got: %AHEAD%
if "%AHEAD%"=="0" (echo Nothing to redo. & goto ship)
git merge-base --is-ancestor origin/main HEAD || (git merge -q --no-edit origin/main || (git merge --abort >nul 2>&1 & echo Could not bring in GitHub's changes. Ask Claude. & exit /b 1))
git reset -q --soft origin/main || (echo The reset failed: nothing was changed. & exit /b 1)
echo Folded back into the working folder: your files are unchanged.
:ship
set "LEYLINE_SHIP_AUTO=1"
call "Ship to GitHub.bat"
exit /b %errorlevel%
