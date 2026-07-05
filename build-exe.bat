@echo off
setlocal

set "ROOT_DIR=%~dp0"
set "SCRIPT=%ROOT_DIR%scripts\package_windows.ps1"

if not exist "%SCRIPT%" (
  echo Cannot find "%SCRIPT%".
  pause
  exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
set "EXIT_CODE=%ERRORLEVEL%"

echo.
if "%EXIT_CODE%"=="0" (
  echo Build finished.
) else (
  echo Build failed with exit code %EXIT_CODE%.
)
pause
exit /b %EXIT_CODE%
