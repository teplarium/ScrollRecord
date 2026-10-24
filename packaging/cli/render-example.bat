@echo off
rem ScrollRecord: renders the included example landing page for desktop and iPhone.
setlocal
chcp 65001 >nul

echo Rendering the example: desktop 16:9 and iPhone 9:16, 1080p, 60 fps.
echo This takes a few minutes. Progress is shown below.
echo.
"%~dp0scrollrecord.exe" render "%~dp0example\lumen.srflow.json"
set "RESULT=%ERRORLEVEL%"
echo.
if "%RESULT%"=="0" (
  echo Done. Opening the videos folder...
  start "" "%~dp0example\renders"
) else (
  echo Rendering failed. Look up the SR code above in docs/ERRORS.md on GitHub.
)
echo.
pause
exit /b %RESULT%
