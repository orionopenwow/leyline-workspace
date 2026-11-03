@echo off
setlocal
title Leyline domain move (check / on / off)
rem The move to leylinelauncher.com (docs\DOMAIN-MOVE.md): domain.sh on the server, with domain_caddy.py and
rem addresses.json next to it. One SSH login.
rem   "Domain move.bat" check   read only: the server's IP, DNS for the four new names, ports 80/443, the website's
rem                             sign-in, and the exact Squarespace DNS records to add
rem   "Domain move.bat" on      backup, the new names (Caddy gets their certificates), self-check each over HTTPS,
rem                             the new names everywhere players read them, then old page loads answer 301
rem                             (automatic rollback on any failure)
rem   "Domain move.bat" off     undo: every name back as it was
cd /d "%~dp0"
set "KEY=%USERPROFILE%\.ssh\openwow"
set "MODE=%~1"
if "%MODE%"=="" set "MODE=check"
set "OK="
if /i "%MODE%"=="check" set "OK=1"
if /i "%MODE%"=="on" set "OK=1"
if /i "%MODE%"=="off" set "OK=1"
if not defined OK (echo Usage: "Domain move.bat" check ^| on ^| off & pause & exit /b 1)
for %%f in (domain.sh domain_caddy.py addresses.json) do if not exist "%%f" (echo %%f is missing next to this file. & pause & exit /b 1)
if not exist "%KEY%" (echo SSH key %KEY% not found. & pause & exit /b 1)
tar -cf - domain.sh domain_caddy.py addresses.json | ssh -i "%KEY%" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "rm -rf /tmp/wu-domain && mkdir -p /tmp/wu-domain && tar -xf - -C /tmp/wu-domain && sed -i 's/\r$//' /tmp/wu-domain/domain.sh /tmp/wu-domain/domain_caddy.py && bash /tmp/wu-domain/domain.sh %MODE% 2>&1 | tee /tmp/wu-domain.log; grep -q -e '^DOMAIN OK' -e '^DOMAIN CHECK OK' -e '^DOMAIN WAITING' /tmp/wu-domain.log"
if errorlevel 1 (echo. & echo It did not report success; see above. & pause & exit /b 1)
echo.
echo Done (%MODE%).
exit /b 0
