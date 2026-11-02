@echo off
setlocal
rem Three Den tests in a row to find what draws the Den green (2 Nov): our own made white texture; the sky left
rem as the game's own; the sky and the zone name both left. Each run's report and picture kept as logs\den-test-<name>.*
set "L=%~dp0logs"
set "LEYLINE_DEN_TEX=made"
call "%~dp0Den test.bat"
copy /y "%L%\den-test-latest.txt" "%L%\den-test-made.txt" >nul 2>&1
copy /y "%L%\den-test-latest.png" "%L%\den-test-made.png" >nul 2>&1
set "LEYLINE_DEN_TEX="
set "LEYLINE_DEN_SKY=0"
call "%~dp0Den test.bat"
copy /y "%L%\den-test-latest.txt" "%L%\den-test-nosky.txt" >nul 2>&1
copy /y "%L%\den-test-latest.png" "%L%\den-test-nosky.png" >nul 2>&1
set "LEYLINE_DEN_AREA=0"
call "%~dp0Den test.bat"
copy /y "%L%\den-test-latest.txt" "%L%\den-test-plain.txt" >nul 2>&1
copy /y "%L%\den-test-latest.png" "%L%\den-test-plain.png" >nul 2>&1
echo DEN TEST VARIANTS DONE
exit /b 0
