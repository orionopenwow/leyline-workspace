@echo off
setlocal
rem 3 Nov: the Den is the game's own grass under the area's own sky (no texture or table of ours to leave out any
rem more), so there are no variants to try: this runs the one Den test.
call "%~dp0Den test.bat"
echo DEN TEST VARIANTS DONE (one test: nothing left to vary)
exit /b 0
