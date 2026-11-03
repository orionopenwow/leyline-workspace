@echo off
rem The realm's scripts reloaded live (the server pass with SERVERPASS_RELOAD=1): for a script that is on the server but
rem silent (2 Nov: ov_reports.lua, .bug and .wu). No restart; the same backup as the server pass.
call "%~dp0Server pass - finder 10 and bug reports.bat" reload
exit /b %errorlevel%
