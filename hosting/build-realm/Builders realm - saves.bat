@echo off
setlocal
title Leyline - Builders' Realm saves
rem The Builders' Realm's save files (.den save <name> in game) to and from this PC (docs/BUILD-REALM.md, "Saves"):
rem   1. every save on the realm is copied here:  Documents\Leyline\Builders Realm saves\
rem   2. every file you put in that folder's "send" subfolder goes to the realm (a file of the same name there is
rem      replaced), then in game: .den load <name>. They are Den files: the launcher's Saved Dens (Add a Den file)
rem      opens them in your own Den too. Only *.den.json files with plain names (letters, digits, - and _) move.
rem One SSH login per direction (root@play.openvanilla.app, %USERPROFILE%\.ssh\openwow). Layout files only: no game data.
set "D=%USERPROFILE%\Documents\Leyline\Builders Realm saves"
if not exist "%D%\send" mkdir "%D%\send"
set "KEY=%USERPROFILE%\.ssh\openwow"
set "HOST=root@play.openvanilla.app"
echo Copying the realm's saves to %D% ...
scp -i "%KEY%" -o StrictHostKeyChecking=accept-new -q "%HOST%:/srv/wrath/build-realm/dens/*.den.json" "%D%\" 2>nul
if errorlevel 1 (echo   none yet, or the realm has never been on.) else (echo   done.)
set "N=0"
for %%F in ("%D%\send\*.den.json") do call :one "%%~F" "%%~nxF"
echo %N% file(s) sent. In game on the Builders' Realm: .den files, then .den load ^<name^>.
echo BUILD REALM SAVES OK
exit /b 0
:one
echo %~2| findstr /r /x "[A-Za-z0-9_-]*\.den\.json" >nul || (echo   skipped %~2: use letters, digits, - and _ only. & exit /b 0)
scp -i "%KEY%" -o StrictHostKeyChecking=accept-new -q "%~1" "%HOST%:/srv/wrath/build-realm/dens/%~2" && (set /a N+=1 >nul & echo   sent %~2) || echo   could not send %~2
exit /b 0
