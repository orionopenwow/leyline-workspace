@echo off
setlocal
title Leyline - Azeroth Survivors at its own address (on)
rem Owner-run. Uploads the browser build (tools\survivors\dist) and the slice you recorded on this PC (tools\survivors\out), then the server checks them, starts sv-play, adds the Caddy site behind the Leyline sign-in, self-checks and only then turns Play on at /wrath/survivors. Rolls back on a failure. Undo: Put Survivors on Leyline - off.bat. Read-only first: Put Survivors on Leyline - check.bat. Without a new upload it re-serves what is there: Put Survivors on Leyline.bat noupload.
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
if /i "%~1"=="noupload" goto pack_plain
rem The build and the slice from this PC: survivors-check first (nothing goes up that does not match the lock and names.sha256).
set PY=py -3
where py >nul 2>&1 || set PY=python
%PY% -I tools\survivors\survivors_check.py all tools\survivors\dist tools\survivors\out\slice tools\survivors\out\names.sha256
if errorlevel 1 (echo. & echo Not uploaded: survivors-check did not pass ^(see above^). Run Record Survivors files.bat first. & pause & exit /b 1)
echo Packing the build and the slice (a few hundred MB; the upload takes a while)...
tar -czf "%TGZ%" --exclude=__pycache__ -s ",^tools/survivors/dist,payload/www," -s ",^tools/survivors/out/slice,payload/slice," -s ",^tools/survivors/out/names.sha256,payload/names.sha256," %PARTS% tools/survivors/dist tools/survivors/out/slice tools/survivors/out/names.sha256 || (echo Packing failed. & pause & exit /b 1)
goto send
:pack_plain
tar -czf "%TGZ%" --exclude=__pycache__ %PARTS% || (echo Packing failed. & pause & exit /b 1)
:send
ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "mkdir -p /srv/survivors && T=$(mktemp -d -p /srv/survivors upload.XXXXXX) && tar -xzf - -C $T && sed 's/\r$//' $T/hosting/survivors/sv-site-up.sh > $T/sv-site-up.sh && SV_SITE_HOST=%SVH% bash $T/sv-site-up.sh $T --go 2>&1 | tee /tmp/sv-site.log; rm -rf $T; grep -q '^SV SITE ON' /tmp/sv-site.log" < "%TGZ%"
set "RC=%ERRORLEVEL%"
del /q "%TGZ%" >nul 2>&1
if not "%RC%"=="0" (echo. & echo It did not finish; see above. & pause & exit /b 1)
pause
exit /b 0
