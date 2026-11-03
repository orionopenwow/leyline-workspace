@echo off
setlocal
title Be a node
rem One click: this PC (or server) carries other players' game connections for the network, as a community relay.
rem It never stores or passes game files or game clients. Needs Docker, a public name (like node.example.org) that
rem points at this machine, and TCP 80, 443 and 41000-41051 open to the internet.
rem   Be a node.bat [PUBLIC-NAME [REGION]]     start (asks when not given)
rem   Be a node.bat off                        stop
rem Where the host kit is: this folder (inside a host kit), host-kit\ next to it, or the repo's hosting\kit.
set "KIT="
if exist "%~dp0docker-compose.volunteer-relay.yml" set "KIT=%~dp0."
if not defined KIT if exist "%~dp0host-kit\docker-compose.volunteer-relay.yml" set "KIT=%~dp0host-kit"
if not defined KIT if exist "%~dp0..\..\hosting\kit\docker-compose.volunteer-relay.yml" set "KIT=%~dp0..\..\hosting\kit"
if not defined KIT (echo The host kit is not next to this file. Unzip the host kit and run this from its folder. & pause & exit /b 1)
if not exist "%KIT%\tools\wurelay" (echo This host kit has no relay code. Use the downloaded host kit ^(or "Make my own Leyline"'s host-kit^). & pause & exit /b 1)
docker version >nul 2>&1 || (echo Docker Desktop is not running. Start it ^(or install it from docker.com^), then run this again. & pause & exit /b 1)
if not defined WU_TOOLS_PREBUILT set "WU_TOOLS_PREBUILT=ghcr.io/orionopenwow/wu-host-tools:1"
set "WU_TOOLS_IMAGE=wu-host-tools:1"
docker image inspect "%WU_TOOLS_IMAGE%" >nul 2>&1 || (docker pull -q "%WU_TOOLS_PREBUILT%" >nul 2>&1 && docker tag "%WU_TOOLS_PREBUILT%" "%WU_TOOLS_IMAGE%")
docker image inspect "%WU_TOOLS_IMAGE%" >nul 2>&1 || docker build -q -t "%WU_TOOLS_IMAGE%" "%KIT%\tools" >nul || (echo Building the helper image failed. Is this PC online? & pause & exit /b 1)
set "SETUP=docker run --rm -v "%KIT%:/kit" -e WU_KIT=/kit %WU_TOOLS_IMAGE% python -m wuhost.setup"
if /i "%~1"=="off" goto off
set "NODE=%~1"
if "%NODE%"=="" set /p "NODE=This machine's public name (like node.example.org): "
if "%NODE%"=="" (echo Nothing typed. & pause & exit /b 1)
set "REGION=%~2"
if "%REGION%"=="" set /p "REGION=Your region: na-east, na-west, sa, eu, africa, me, asia or oce (Enter to skip): "
set "RARG="
if not "%REGION%"=="" set "RARG=--region %REGION%"
%SETUP% volunteer-relay --host %NODE% %RARG% || (echo Setup failed ^(see above^). & pause & exit /b 1)
docker compose -f "%KIT%\docker-compose.volunteer-relay.yml" --project-directory "%KIT%" up -d --build || (echo Starting the node failed ^(see above^). & pause & exit /b 1)
echo.
echo Your node is starting. Stop it any time with: "Be a node.bat" off
echo Until the network opens community relays to everyone, only hosts who pick it send players here.
pause & exit /b 0
:off
%SETUP% volunteer-relay --off
docker compose -f "%KIT%\docker-compose.volunteer-relay.yml" --project-directory "%KIT%" down
echo Your node is off.
pause & exit /b 0
