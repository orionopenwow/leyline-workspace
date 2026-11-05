@echo off
rem Leyline Control step: the realm's character creation set to today's retail race/class pairs (83).
rem The same as "Fix character create.bat" modern (backup, world tables, settings, world server restart, check).
call "%~dp0Fix character create.bat" modern
exit /b %errorlevel%
