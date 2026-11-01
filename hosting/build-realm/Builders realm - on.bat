@echo off
setlocal
title Leyline - Builders' Realm - on
rem Turns the Builders' Realm on next to the main realm (never restarts it): backup, its databases, its login and world
rem servers, a self-check, then the Leyline list through the relay. Any failure rolls back. Safe to run again.
rem docs/BUILD-REALM.md. One SSH login (root@play.openvanilla.app, %USERPROFILE%\.ssh\openwow); our own code only goes over
rem stdin as one small tarball (no game data, no secrets), and hosting/build-realm/build-realm.sh does the work.
cd /d "%~dp0..\.."
if not exist "hosting\build-realm\build-realm.sh" (echo hosting\build-realm\build-realm.sh is missing. & pause & exit /b 1)
where tar >nul 2>&1 || (echo tar.exe is missing ^(Windows 10 1803 or later has it^). & pause & exit /b 1)
set "TGZ=%TEMP%\wu-build-realm-src.tgz"
if exist "%TGZ%" del /q "%TGZ%"
tar -czf "%TGZ%" --exclude=__pycache__ --exclude=*.pyc --exclude=state hosting/build-realm hosting/kit/tools/wunet hosting/kit/tools/wuhost hosting/kit/tools/Dockerfile realm/profiles/build.env realm/custom/lua_scripts/ov_buildrealm.lua realm/custom/lua_scripts/ov_den.lua || (echo Packing failed. & pause & exit /b 1)
ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "T=$(mktemp -d) && tar -xzf - -C $T && sed 's/\r$//' $T/hosting/build-realm/build-realm.sh > $T/build-realm.sh && bash $T/build-realm.sh $T --go 2>&1 | tee /tmp/wu-build-realm.log; rm -rf $T; grep -q '^BUILD REALM ON' /tmp/wu-build-realm.log" < "%TGZ%"
set "RC=%ERRORLEVEL%"
del /q "%TGZ%" >nul 2>&1
if not "%RC%"=="0" (echo. & echo It did not finish; see above. Nothing is left half-done: it rolled back. & pause & exit /b 1)
exit /b 0
