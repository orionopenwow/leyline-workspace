@echo off
setlocal EnableDelayedExpansion
title Leyline - update the public Leyline Solo repo (github.com/orionopenwow/leyline-solo)
rem Leyline Solo's own public repo (docs\LEYLINE-SOLO.md): tools\solo-repo\export.py copies the launcher crate with
rem the solo feature on, the Solo window (launcher\ui-solo), tools\solo, the installer and engine workflows, README,
rem LICENSE and NOTICE into the clean clone .repos\leyline-solo-clean (only files git tracks; it refuses game data,
rem secrets, logs, brand words and version numbers in the README), then commits as Orion and pushes to main.
rem The owner makes the empty repo on GitHub once: this never creates it, never force-pushes, never touches its tags or
rem releases, and never touches the workspace's own git (it only reads the commit id). Safe to run again: nothing
rem changed = nothing committed.
set "WU=%~dp0..\.."
for %%I in ("%WU%") do set "WU=%%~fI"
set "LS=%WU%\.repos\leyline-solo-clean"
set "SLUG=orionopenwow/leyline-solo"
set "URL=https://github.com/%SLUG%.git"
set GIT_TERMINAL_PROMPT=0

set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & goto :fail)
where git >nul 2>&1 || (echo Git is not installed: https://git-scm.com/download/win & goto :fail)
where gh >nul 2>&1 || (echo The GitHub CLI is not installed: winget install GitHub.cli  ^(then: gh auth login^) & goto :fail)

echo [1/4] GitHub sign-in and the repo...
set "WHO="
for /f "delims=" %%U in ('gh api user --jq .login 2^>nul') do set "WHO=%%U"
if not defined WHO (echo   Not signed in to the GitHub CLI. Run once: gh auth login  ^(as orionopenwow^) & goto :fail)
if /i "!WHO!"=="Chumpton" (echo   The GitHub CLI is signed in as !WHO!. Leyline is published as orionopenwow only: gh auth switch & goto :fail)
echo   signed in as !WHO!
gh repo view %SLUG% --json name >nul 2>&1 || (echo   create orionopenwow/leyline-solo on GitHub ^(empty, public: no README, no licence^) then press again. & goto :fail)

echo [2/4] The clean clone: %LS%
if exist "%LS%\.git" goto :have_clone
git clone -q "%URL%" "%LS%" 2>nul || (echo   git clone failed. Nothing was pushed. & goto :fail)
:have_clone
cd /d "%LS%" || goto :fail
set "ORIGIN="
for /f "delims=" %%O in ('git remote get-url origin 2^>nul') do set "ORIGIN=%%O"
echo !ORIGIN! | findstr /i /c:"github.com/%SLUG%" /c:"github.com:%SLUG%" >nul || (echo   origin is !ORIGIN!, not %SLUG%. Nothing was pushed. Ask Claude. & goto :fail)
git fetch -q origin 2>nul
git rev-parse -q --verify origin/main >nul 2>&1 || goto :export
git rev-parse -q --verify HEAD >nul 2>&1 || (git reset -q origin/main & goto :export)
git merge-base --is-ancestor origin/main HEAD 2>nul && goto :export
git merge -q --no-edit origin/main || (
  git merge --abort >nul 2>&1
  echo   GitHub's leyline-solo and this folder changed the same lines. Nothing was pushed. Ask Claude.
  goto :fail
)

:export
echo [3/4] Copying Leyline Solo from %WU%
%PY% "%WU%\tools\solo-repo\export.py" "%LS%" --source "%WU%" || (echo   The export stopped ^(see above^). Nothing was committed. & goto :fail)
git add -A || (echo   git add failed. & goto :fail)
for /f "delims=" %%X in ('%PY% "%WU%\tools\solo-repo\export.py" --list-exec --source "%WU%"') do git update-index --chmod=+x -- "%%X" >nul 2>&1
git diff --cached --quiet && (echo   nothing changed since the last copy) && goto :push
for /f "delims=" %%L in ('git --no-pager diff --cached --shortstat') do echo   %%L
set "REV="
for /f "delims=" %%R in ('git -C "%WU%" rev-parse --short HEAD 2^>nul') do set "REV=%%R"
if defined REV (set "MSG=Leyline Solo update !REV!") else set "MSG=Leyline Solo update"
rem Saved as Orion, the owner, whatever the PC's identity is
git config user.name "Orion"
git config user.email "orionopenwow@users.noreply.github.com"
git commit -q -m "!MSG!" || (echo   Commit failed. & goto :fail)
echo   committed: !MSG!

:push
git rev-parse -q --verify HEAD >nul 2>&1 || (echo   Nothing to push yet. & goto :ok)
echo [4/4] Pushing to GitHub (%SLUG%, main)...
git push -q -u origin HEAD:main || (echo   Push failed: check the sign-in ^(gh auth status^). Nothing was forced. & goto :fail)

:ok
echo.
echo LEYLINE SOLO OK: https://github.com/%SLUG%  (the installer build: https://github.com/%SLUG%/actions)
pause
exit /b 0

:fail
echo.
echo LEYLINE SOLO FAILED
pause
exit /b 1
