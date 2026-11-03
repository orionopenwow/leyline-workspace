@echo off
setlocal
title Start my own server
rem One click: your own realm on this PC, from your own game client, with the host kit (Docker Desktop needed).
rem It starts private (friends with your invite link); "Realm tools.bat public" lists it later.
rem Nothing is downloaded but the open-source server software: map data is read from YOUR game folder, on this PC.
rem Where the host kit is: this folder (inside a host kit), host-kit\ next to it, or the repo's hosting\kit.
set "KIT="
if exist "%~dp0Host a Leyline realm.bat" set "KIT=%~dp0."
if not defined KIT if exist "%~dp0host-kit\Host a Leyline realm.bat" set "KIT=%~dp0host-kit"
if not defined KIT if exist "%~dp0..\..\hosting\kit\Host a Leyline realm.bat" set "KIT=%~dp0..\..\hosting\kit"
if not defined KIT (echo The host kit is not next to this file. Unzip the host kit and run this from its folder. & pause & exit /b 1)
if not defined WU_VISIBILITY set "WU_VISIBILITY=private"
echo Your own server: it asks for a name, a template and your game folder, then starts.
echo The first start takes 5 to 20 minutes. Close this window any time after "Your realm is up".
echo.
call "%KIT%\Host a Leyline realm.bat" %*
