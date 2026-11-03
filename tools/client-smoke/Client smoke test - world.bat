@echo off
rem Client smoke test, on into the world with your last character (Client smoke test.bat world).
call "%~dp0Client smoke test.bat" world
exit /b %ERRORLEVEL%
