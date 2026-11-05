@echo off
setlocal
title Leyline - reel-make (cuts the reels)
rem The reel kit's edit (tools/reels/edit.py): every recorded take cut into the reels (9:16), the flow videos and the
rem launch video (16:9 and 9:16), each with an .srt, a contact sheet and a GIF for Reddit, in Leyline's look.
rem Out: tools\reels\out\<reel>\ (not in git). A reel whose words are still TODO(owner) wears a DRAFT tag.
rem Nothing is posted anywhere: the videos wait for you.
rem   Reel make.bat                    every reel (takes not recorded yet are left out)
rem   Reel make.bat den-carry          one reel;  --placeholders shows a card for each missing take
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
%PY% -c "import PIL" >nul 2>&1 || (echo Getting Pillow... & %PY% -m pip install --user --quiet pillow)
%PY% -I "tools\reels\edit.py" %*
set "CODE=%ERRORLEVEL%"
if exist "tools\reels\out" echo The videos: %CD%\tools\reels\out
exit /b %CODE%
