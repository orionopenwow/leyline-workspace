@echo off
rem den-test with a friend: a test login joins your Den through the friends' listener and your game sees them.
call "%~dp0Den test.bat" invite
exit /b %ERRORLEVEL%
