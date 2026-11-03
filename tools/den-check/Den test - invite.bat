@echo off
rem den-test with a friend: the same as Den test.bat now (its smoke test always invites a test friend).
call "%~dp0Den test.bat" invite
exit /b %ERRORLEVEL%
