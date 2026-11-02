@echo off
setlocal
title Leyline - release audit (read-only, about 10 minutes)
rem Every feature checked as a new player meets it: the website, the downloads, the server (over the same SSH login
rem the check step uses), Wrath Unleashed played by a headless test client with the audit's own AUDIT account,
rem Your Den / Play solo and Invite friends with Den Lite on this PC, hosting, the launcher, Looks, the forum and
rem Azeroth Survivors. Read only, except the AUDIT account's own test characters and a Den Lite temp folder.
rem The report: logs\audit-latest.html (opened at the end) and logs\audit-latest.txt. docs\AUDIT.md says more.
cd /d "%~dp0..\.."
if not exist logs mkdir logs
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: winget install Python.Python.3.12 & exit /b 2)
%PY% -I tools\audit\audit.py %*
set "RC=%ERRORLEVEL%"
if exist logs\audit-latest.html if not defined LEYLINE_AUDIT_NO_OPEN start "" "logs\audit-latest.html"
exit /b %RC%
