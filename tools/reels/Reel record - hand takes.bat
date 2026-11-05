@echo off
setlocal
title Leyline - reel-record-hand (you play, it records)
rem The takes only you can play (scripts/takes.json source "hand": a community realm and back, Single player with its
rem bots, the HD pack before and after, the launcher's own window): for each, a box says what to do and for how long;
rem OK starts the recording, it stops by itself; Cancel skips that take. Takes: tools\reels\takes\. Then reel-make.
cd /d "%~dp0..\.."
set "PY="
where py >nul 2>&1 && set "PY=py -3"
if not defined PY where python >nul 2>&1 && set "PY=python"
if not defined PY (echo Python 3 is needed: https://www.python.org/downloads/ & exit /b 1)
rem ffmpeg records and cuts the videos: winget puts it in (free, from the ffmpeg builds winget lists) when it is missing
where ffmpeg >nul 2>&1 || (
  if not exist "%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg*" (
    where winget >nul 2>&1 && (echo Getting ffmpeg with winget... & winget install -e --id Gyan.FFmpeg --accept-source-agreements --accept-package-agreements --disable-interactivity >nul)
  )
)
%PY% -I "tools\reels\record.py" --hand %*
exit /b %ERRORLEVEL%
