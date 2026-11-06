@echo off
setlocal EnableDelayedExpansion
title Leyline - update the public Leyline-Mobile repo (github.com/orionopenwow/Leyline-Mobile)
rem The phone release's own repo (docs\MOBILE-RELEASE.md): mobile_export.py copies the touch layer, phone solo's page
rem side, a mirror of the phone app's files, the screenshots, the guides and the Android workflow into
rem .repos\leyline-mobile (an allow-list; it refuses game data, secrets, logs, brand words and version numbers), then
rem commits as Orion and pushes. The app is built from the Leyline repo at the commit .repos\leyline-public is on
rem (LEYLINE_REF), so press ship (which runs the leyline step) first.
rem The owner makes the empty repo on GitHub once: this never creates it, never force-pushes, and never touches the
rem workspace's own git (it only reads commit ids). Safe to run again: nothing changed = nothing committed.
set "WU=%~dp0..\.."
for %%I in ("%WU%") do set "WU=%%~fI"
set "LM=%WU%\.repos\leyline-mobile"
set "LP=%WU%\.repos\leyline-public"
set "SLUG=orionopenwow/Leyline-Mobile"
set "URL=https://github.com/%SLUG%.git"
set GIT_TERMINAL_PROMPT=0

set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & goto :fail)
where git >nul 2>&1 || (echo Git is not installed: https://git-scm.com/download/win & goto :fail)
where gh >nul 2>&1 || (echo The GitHub CLI is not installed: winget install GitHub.cli  ^(then: gh auth login^) & goto :fail)

echo [1/5] GitHub sign-in and the repo...
set "WHO="
for /f "delims=" %%U in ('gh api user --jq .login 2^>nul') do set "WHO=%%U"
if not defined WHO (echo   Not signed in to the GitHub CLI. Run once: gh auth login  ^(as orionopenwow^) & goto :fail)
if /i "!WHO!"=="Chumpton" (echo   The GitHub CLI is signed in as !WHO!. Leyline is published as orionopenwow only: gh auth switch & goto :fail)
echo   signed in as !WHO!
gh repo view %SLUG% --json name >nul 2>&1 || (echo   %SLUG% is not on GitHub yet. Make it once on github.com ^(public, empty: no README, no licence^), then press leyline-mobile again. & goto :fail)

echo [2/5] The Leyline commit the app is built from...
set "REF="
if exist "%LP%\.git" for /f "delims=" %%R in ('git -C "%LP%" rev-parse HEAD 2^>nul') do set "REF=%%R"
if not defined REF for /f "delims=" %%R in ('gh api repos/orionopenwow/Leyline/commits/main --jq .sha 2^>nul') do set "REF=%%R"
if not defined REF (echo   Could not read the Leyline repo's commit. Press ship first. & goto :fail)
echo   Leyline at !REF!

echo [3/5] The Leyline-Mobile folder: %LM%
if not exist "%LM%" mkdir "%LM%" || (echo   Could not make %LM% & goto :fail)
cd /d "%LM%" || goto :fail
if not exist ".git" (git init -q -b main || (echo   git init failed. & goto :fail))
set "ORIGIN="
for /f "delims=" %%O in ('git remote get-url origin 2^>nul') do set "ORIGIN=%%O"
if not defined ORIGIN (
  git remote add origin "%URL%" || (echo   Could not set the remote. & goto :fail)
  set "ORIGIN=%URL%"
)
echo !ORIGIN! | findstr /i /c:"github.com/%SLUG%" /c:"github.com:%SLUG%" >nul || (echo   origin is !ORIGIN!, not %SLUG%. Nothing was pushed. Ask Claude. & goto :fail)
git fetch -q origin 2>nul
git rev-parse -q --verify origin/main >nul 2>&1 || goto :export
git rev-parse -q --verify HEAD >nul 2>&1 || (git reset -q origin/main & goto :export)
git merge-base --is-ancestor origin/main HEAD 2>nul && goto :export
git merge -q --no-edit origin/main || (
  git merge --abort >nul 2>&1
  echo   GitHub's Leyline-Mobile and this folder changed the same lines. Nothing was pushed. Ask Claude.
  goto :fail
)

:export
echo [4/5] Copying the phone release from %WU%
%PY% "%WU%\tools\leyline-repo\mobile_export.py" "%LM%" --source "%WU%" --leyline-ref !REF! || (echo   The export stopped ^(see above^). Nothing was committed. & goto :fail)
git add -A || (echo   git add failed. & goto :fail)
for /f "delims=" %%X in ('%PY% "%WU%\tools\leyline-repo\mobile_export.py" --list-exec --source "%WU%"') do git update-index --chmod=+x -- "%%X" >nul 2>&1
git diff --cached --quiet && (echo   nothing changed since the last copy) && goto :push
for /f "delims=" %%L in ('git --no-pager diff --cached --shortstat') do echo   %%L
set "REV="
for /f "delims=" %%R in ('git -C "%WU%" rev-parse --short HEAD 2^>nul') do set "REV=%%R"
if defined REV (set "MSG=Leyline-Mobile update !REV!") else set "MSG=Leyline-Mobile update"
rem Saved as Orion, the owner, whatever the PC's identity is
git config user.name "Orion"
git config user.email "orionopenwow@users.noreply.github.com"
git commit -q -m "!MSG!" || (echo   Commit failed. & goto :fail)
echo   committed: !MSG!

:push
git rev-parse -q --verify HEAD >nul 2>&1 || (echo   Nothing to push yet. & goto :ok)
echo [5/5] Pushing to GitHub (%SLUG%, public)...
git push -q -u origin HEAD:main || (echo   Push failed: check the sign-in ^(gh auth status^). Nothing was forced. & goto :fail)

:ok
echo.
echo LEYLINE MOBILE OK: https://github.com/%SLUG%
pause
exit /b 0

:fail
echo.
echo LEYLINE MOBILE FAILED
pause
exit /b 1
