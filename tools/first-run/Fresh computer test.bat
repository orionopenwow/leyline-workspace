@echo off
setlocal
title Leyline - fresh computer test (every feature, a brand-new launcher, through to in the game)
rem The fresh computer test (docs/FRESH-COMPUTER-TEST.md): today's launcher download in an empty data folder (nothing
rem of your own launcher is used or changed). It checks this computer (WebView2, the Visual C++ runtime, the addons
rem pack and Den Lite downloads), adds your game folders, drops a folder, a zip, a link, a magnet link and a .torrent
rem on the game folder box, logs a test Hero Key in by file, out, and in by paste, logs in on the website (its cookie),
rem then PLAY on Wrath Unleashed into the world, a community realm and back, Your Den, loadouts Clean and back.
rem The test account is closed and the temp folders removed after, always; your game folder's files are put back.
rem PASS / FAIL per numbered check with the fix. Logs: tools\first-run\logs\first-run-latest.txt and the .png files
rem   Fresh computer test.bat                  all of it
rem   Fresh computer test.bat --den-features   with the Den editor's feature test
rem   Fresh computer test.bat --no-game        no game folder (the computer, log in, the website, the drops)
rem   Fresh computer test.bat --keep           keep the fresh folders (to open that launcher by hand after)
rem In a brand-new Windows: "Fresh computer test - sandbox.bat" (Windows Sandbox).
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
%PY% -I "tools\first-run\first_run.py" run %*
exit /b %ERRORLEVEL%
