@echo off
setlocal
title Leyline - Vanilla Remastered self-test (a fresh TEST account)
rem Self-check: the page, the slice behind it, brand words, and a fresh TEST account made through the realm's own account command, logged in (directly and through the relay), then deleted. Never the owner's account.
cd /d "%~dp0..\.."
if not exist "hosting\vanilla-remastered\vr-site-up.sh" (echo hosting\vanilla-remastered\vr-site-up.sh is missing. & pause & exit /b 1)
where tar >nul 2>&1 || (echo tar.exe is missing ^(Windows 10 1803 or later has it^). & pause & exit /b 1)
set "TGZ=%TEMP%\wu-vr-site-src-%RANDOM%%RANDOM%.tgz"
if exist "%TGZ%" del /q "%TGZ%"
tar -czf "%TGZ%" --exclude=__pycache__ --exclude=*.pyc hosting/vanilla-remastered/vr-site-up.sh hosting/vanilla-remastered/probe112.py stream/server/gamedata-caddy.py hosting/kit/tools/wunet || (echo Packing failed. & pause & exit /b 1)
ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "T=$(mktemp -d) && tar -xzf - -C $T && sed 's/\r$//' $T/hosting/vanilla-remastered/vr-site-up.sh > $T/vr-site-up.sh && bash $T/vr-site-up.sh $T --test 2>&1 | tee /tmp/vr-site.log; rm -rf $T; grep -q '^VR SITE TEST OK' /tmp/vr-site.log" < "%TGZ%"
set "RC=%ERRORLEVEL%"
del /q "%TGZ%" >nul 2>&1
if not "%RC%"=="0" (echo. & echo It did not finish; see above. & exit /b 1)
exit /b 0
