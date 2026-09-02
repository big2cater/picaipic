@echo off
setlocal

rem Package PicAiPic for Windows.
rem Always pass -Clean so icon.ico (from favicon1.ico) is re-linked into the EXE.
rem package_windows.ps1 also regenerates icons from repo-root favicon1.ico before build.

set "ROOT_DIR=%~dp0"
set "SCRIPT=%ROOT_DIR%scripts\package_windows.ps1"

if not exist "%SCRIPT%" (
  echo Cannot find "%SCRIPT%".
  pause
  exit /b 1
)

if not exist "%ROOT_DIR%favicon1.ico" (
  echo WARNING: favicon1.ico not found at repo root.
  echo Taskbar/exe icons may be wrong. Place the brand ICO as favicon1.ico then rebuild.
  echo.
)

echo Building with -Clean ^(regenerate icons from favicon1.ico + cargo clean -p PicAiPic^)...
echo Extra args: %*
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" -Clean %*
set "EXIT_CODE=%ERRORLEVEL%"

echo.
if "%EXIT_CODE%"=="0" (
  echo Build finished.
  echo Main EXE: %ROOT_DIR%src-tauri\target\release\PicAiPic.exe
  echo Installers: %ROOT_DIR%src-tauri\target\release\bundle
  echo.
  echo Before testing an installer, fully exit every running PicAiPic process.
  echo For same-version MSI retests, uninstall the existing app first or bump the version.
) else (
  echo Build failed with exit code %EXIT_CODE%.
)
pause
exit /b %EXIT_CODE%
