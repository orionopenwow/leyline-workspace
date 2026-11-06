@echo off
setlocal
title Leyline - Phone Lab
rem The newest Phone Lab session (docs/MOBILE-WEB-OVERHAUL.md, stream/phonelab.py): every tap on the owner's phone (where,
rem on what, the touch events in order and their timings, full screen asks, errors), read on the server and printed here.
rem The session and its sketches are copied to tools\phonelab\live\ (not in git). Read only. Another session: set CODE=ABC123 first.
cd /d "%~dp0"
if not exist phonelab.py (echo phonelab.py is missing next to this file. & exit /b 1)
if not exist live mkdir live
set "KEY=%USERPROFILE%\.ssh\openwow"
set "HOST=root@play.openvanilla.app"
set "SSHO=-o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30"
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
type phonelab.py | ssh -i "%KEY%" %SSHO% %HOST% "tr -d '\r' > /tmp/wu-phonelab.py && python3 -I /tmp/wu-phonelab.py --dir /srv/wrath/portal-data/phonelab --latest --code %CODE% --copy /tmp/wu-phonelab-latest; rm -f /tmp/wu-phonelab.py" > live\pull.txt
type live\pull.txt
findstr /b /c:"PHONELAB OK" live\pull.txt >nul || (echo No Phone Lab session to read: start one on the phone at leylinelauncher.com/wrath/phonelab. & exit /b 1)
scp -q -i "%KEY%" %SSHO% %HOST%:/tmp/wu-phonelab-latest.jsonl %HOST%:/tmp/wu-phonelab-latest.json live\ || (echo Could not copy the session. & exit /b 1)
if exist live\shots rmdir /s /q live\shots
if defined PY %PY% -I phonelab.py --file live\wu-phonelab-latest.jsonl --shots live\shots > live\latest.txt && echo Saved tools\phonelab\live\latest.txt and the sketches in tools\phonelab\live\shots
exit /b 0
