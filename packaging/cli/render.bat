@echo off
rem ScrollRecord: drag a scenario file (*.srflow.json) onto this file to render it.
rem The videos are written to a "renders" folder next to the scenario file.
setlocal
chcp 65001 >nul

if "%~1"=="" (
  echo Drag a scenario file ^(*.srflow.json^) onto render.bat to render it.
  echo Or run:  scrollrecord.exe render path\to\scenario.srflow.json
  echo.
  pause
  exit /b 1
)

"%~dp0scrollrecord.exe" render "%~1"
set "RESULT=%ERRORLEVEL%"
echo.
if "%RESULT%"=="0" (
  echo Done. The videos are in the "renders" folder next to the scenario.
  start "" "%~dp1renders"
) else (
  echo Rendering failed. Look up the SR code above in docs/ERRORS.md on GitHub.
)
echo.
pause
exit /b %RESULT%
