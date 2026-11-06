@echo off
setlocal
title Leyline - realm image: the newest realm build live on Wrath Unleashed
rem Control step "realm-image" (docs/BOSS-FIGHTS.md). Puts the realm image GitHub built after the last ship live:
rem realm\realm-image.sh on the server pulls ghcr.io/orionopenwow/wu-ac-wotlk-*:stable, runs the backup, keeps the
rem running images as :rollback, warns in game for 5 minutes, restarts, and checks the world server is ready with
rem mod-leyline-bossfights on; anything wrong puts the old images back by itself. One SSH login.
rem   "Update realm image.bat" now     without the 5-minute warning
cd /d "%~dp0"
set "KEY=%USERPROFILE%\.ssh\openwow"
set "WARN=5"
if /i "%~1"=="now" set "WARN=0"
if not exist realm-image.sh (echo realm-image.sh is missing next to this file. & pause & exit /b 1)
ssh -i "%KEY%" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "cat > /srv/wrath/realm-image.sh && sed -i 's/\r$//' /srv/wrath/realm-image.sh && REALM_IMAGE_WARN_MIN=%WARN% bash /srv/wrath/realm-image.sh 2>&1 | tee /tmp/wu-realm-image.log; grep -q '^REALM IMAGE OK' /tmp/wu-realm-image.log" < realm-image.sh
if errorlevel 1 (echo. & echo It did not report success; see above. & exit /b 1)
echo Done.
exit /b 0
