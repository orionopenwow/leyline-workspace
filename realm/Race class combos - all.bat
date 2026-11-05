@echo off
rem Leyline Control step: the realm's character creation set to every race with every class (100 pairs).
rem The same as "Fix character create.bat" all (backup, world tables, settings, world server restart, check).
call "%~dp0Fix character create.bat" all
exit /b %errorlevel%
