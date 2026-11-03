@echo off
setlocal
title Leyline - feedback digest
rem The feedback digest (stream\feedback.py, docs\ADMIN-CONSOLE.md "Review"): what players said in bug reports, the
rem forum and the Discord's bug channel, in themes with a mood score, and the quietest window for pushing updates.
rem Read-only on everything it reads: the server writes only the digest (portal-data\ops, HQ > Review > Feedback
rem shows it) and this copies it and the upgrades waiting for the owner to tools\feedback\live\ (not in git: player
rem text stays off GitHub). The Control step "feedback". Nothing is deployed, approved or posted.
cd /d "%~dp0"
set "PY=..\..\stream\feedback.py"
if not exist "%PY%" (echo stream\feedback.py is missing. & exit /b 1)
if not exist live mkdir live
set "KEY=%USERPROFILE%\.ssh\openwow"
set "HOST=root@play.openvanilla.app"
set "SSHO=-o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30"
type "%PY%" | ssh -i "%KEY%" %SSHO% %HOST% "tr -d '\r' > /tmp/wu-feedback.py && python3 -I /tmp/wu-feedback.py digest --data /srv/wrath/portal-data --wu /srv/wrath --out /srv/wrath/portal-data/ops; s=$?; rm -f /tmp/wu-feedback.py; chown -R --reference=/srv/wrath/portal-data /srv/wrath/portal-data/ops 2>/dev/null; exit $s" > live\run.txt
type live\run.txt
findstr /b /c:"FEEDBACK OK" live\run.txt >nul || (echo It did not finish; see tools\feedback\live\run.txt. & exit /b 1)
scp -q -i "%KEY%" %SSHO% %HOST%:/srv/wrath/portal-data/ops/feedback-digest.json live\ || (echo Could not copy the digest. & exit /b 1)
scp -q -i "%KEY%" %SSHO% %HOST%:/srv/wrath/portal-data/admin/upgrades.json live\ 2>nul
echo Saved tools\feedback\live\feedback-digest.json
exit /b 0
