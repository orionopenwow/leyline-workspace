@echo off
setlocal
title Leyline - take the gliding copy's rows away (den-glide-undo)
rem Den Lite's glide-rows --undo: our Data\patch-S.MPQ and the marker taken away (someone else's archive is never
rem touched); the copy in hand is a gameobject made anew again. Close the game first.
call "%~dp0Den glide rows.bat" --undo
exit /b %ERRORLEVEL%
