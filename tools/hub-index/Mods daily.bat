@echo off
rem The daily mods and addons run on this PC (Autopilot step "mods-daily"; docs\MODS-DAILY.md). It chains the tools
rem that already exist and decides nothing on its own:
rem   1. tools\addon-curate\curate.py run   every addon seed: new ones and new upstream versions are curated and
rem                                          hashed into mods\upstream.json, and they WAIT for review (never approved here)
rem   2. tools\hub-index\hub_index.py thumbs --new   preview images for catalog entries that have none yet
rem   3. tools\hub-index\hub_index.py check  and  tools\looks\looks.py check   the lists still pass their rules
rem   4. tools\addon-curate\curate.py list   what is waiting, for the Admin Console's review
rem Scouting new projects and the index build and gate run in the cloud every day (.github\workflows\scout.yml and
rem hub-index.yml), so they are not repeated here. Afterwards: review in the Admin Console, then Ship and "signin".
rem Needs Python 3.9+ and git; Pillow for the thumbnails (installed for your user if missing). Safe to run again.
setlocal
cd /d "%~dp0..\.."
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)" >nul 2>&1 || (echo Python 3.9 or newer is needed: https://www.python.org/downloads/ & exit /b 1)
where git >nul 2>&1 || (echo git is needed: https://git-scm.com/download/win & exit /b 1)
echo [1/4] Curating addons from their authors (new ones wait for review)...
%PY% -I "tools\addon-curate\curate.py" run
if errorlevel 1 (echo Curation stopped: see the lines above. & exit /b 1)
echo [2/4] Preview images for new catalog entries...
%PY% -c "import PIL" >nul 2>&1
if errorlevel 1 %PY% -m pip install --user --quiet --disable-pip-version-check pillow
rem Not -I: Pillow may be in the user's site-packages, which -I would hide.
%PY% -E "tools\hub-index\hub_index.py" thumbs --new
if errorlevel 1 echo Thumbnails skipped this time: see the lines above (the rest still runs).
echo [3/4] Checking the lists...
%PY% -E "tools\hub-index\hub_index.py" check
if errorlevel 1 (echo The index check failed: nothing to ship until it passes. & exit /b 1)
%PY% -I "tools\looks\looks.py" check
if errorlevel 1 (echo The looks check failed: nothing to ship until it passes. & exit /b 1)
echo [4/4] Waiting for your review:
%PY% -I "tools\addon-curate\curate.py" list
echo.
echo Done. Nothing was approved. Review in the Admin Console (Mods), then Ship (save to GitHub) and "signin".
exit /b 0
