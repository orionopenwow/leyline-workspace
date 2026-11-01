@echo off
setlocal
title Leyline - Azeroth Survivors at its own address (check)
rem Read-only: what Put Survivors on Leyline.bat would do on the server (the address's DNS, what is installed, what is missing). Uploads nothing but the scripts.
rem The address: hosting\survivors\address.txt (its DNS A record must point at the server). docs\SURVIVORS.md.
cd /d "%~dp0..\.."
if not exist "hosting\survivors\sv-site-up.sh" (echo hosting\survivors\sv-site-up.sh is missing. & pause & exit /b 1)
where tar >nul 2>&1 || (echo tar.exe is missing ^(Windows 10 1803 or later has it^). & pause & exit /b 1)
set "SVH="
for /f "usebackq eol=# tokens=*" %%a in ("hosting\survivors\address.txt") do if not defined SVH set "SVH=%%a"
if not defined SVH (echo No address in hosting\survivors\address.txt. & pause & exit /b 1)
echo %SVH%| findstr /r /x /c:"[a-z0-9][a-z0-9.-]*" >nul || (echo The address "%SVH%" in hosting\survivors\address.txt is not a host name. & pause & exit /b 1)
echo Azeroth Survivors' address: %SVH%
set "TGZ=%TEMP%\leyline-sv-%RANDOM%%RANDOM%.tgz"
if exist "%TGZ%" del /q "%TGZ%"
set "PARTS=hosting/survivors/sv-site-up.sh hosting/survivors/survivors.caddy hosting/survivors/docker-compose.yml tools/survivors/survivors_check.py stream/server/gamedata-caddy.py stream/survivors/upstream.lock.json"
tar -czf "%TGZ%" --exclude=__pycache__ %PARTS% || (echo Packing failed. & pause & exit /b 1)
ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "mkdir -p /srv/survivors && T=$(mktemp -d -p /srv/survivors upload.XXXXXX) && tar -xzf - -C $T && sed 's/\r$//' $T/hosting/survivors/sv-site-up.sh > $T/sv-site-up.sh && SV_SITE_HOST=%SVH% bash $T/sv-site-up.sh $T --check 2>&1 | tee /tmp/sv-site.log; rm -rf $T; grep -q '^SV SITE DRY' /tmp/sv-site.log" < "%TGZ%"
set "RC=%ERRORLEVEL%"
del /q "%TGZ%" >nul 2>&1
if not "%RC%"=="0" (echo. & echo It did not finish; see above. & pause & exit /b 1)
pause
exit /b 0
