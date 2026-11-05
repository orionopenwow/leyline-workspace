@echo off
setlocal
title Leyline - Spike test
rem Spike test: about a minute of moderate load on the live server's own services (the website program, the game
rem files, Caddy), measured from inside the server, then what each one could take. Writes nothing, uses no account.
rem Same box and key as the other server steps (root@play.openvanilla.app, %USERPROFILE%\.ssh\openwow). One SSH login.
cd /d "%~dp0"
for %%f in (spike-test.sh spike_test.py) do if not exist "%%f" (echo %%f is missing next to this file. & pause & exit /b 1)
if not exist live mkdir live
for /f %%d in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmm"') do set "D=%%d"
set "OUT=live\spike-%D%.txt"
tar -cf - spike-test.sh spike_test.py | ssh -i "%USERPROFILE%\.ssh\openwow" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 root@play.openvanilla.app "rm -rf /tmp/wu-spike && mkdir -p /tmp/wu-spike && tar -xf - -C /tmp/wu-spike && sed -i 's/\r$//' /tmp/wu-spike/*.sh /tmp/wu-spike/*.py && bash /tmp/wu-spike/spike-test.sh; rc=$?; rm -rf /tmp/wu-spike; exit $rc" > "%OUT%" 2>&1
type "%OUT%"
findstr /b /c:"SPIKE TEST OK" "%OUT%" >nul || (echo It did not finish; see %OUT%. & exit /b 1)
echo Saved %OUT%
exit /b 0
